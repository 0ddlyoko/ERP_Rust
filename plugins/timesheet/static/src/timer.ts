import { Component, effect, inject, props, state, t } from "trame";
import { Notifications } from "@web/core/notifications";
import { Orm } from "@web/core/orm";
import { FormDialog } from "@web/views/form/form_dialog";
import { type Choice } from "@web/views/widgets/record_search";
import { widgetProps, widgets, Widget } from "@web/views/widgets/widget";
import { services, systray } from "@web/web_client/systray";

/** The caller's timer, as `timesheet_timer` describes it. */
interface TimerStatus {
    running: boolean;
    started?: string;
    project?: Choice | null;
    task?: Choice | null;
    description?: string | null;
}

/**
 * The user's timer, one for the whole page: what the tray and the task forms show and act on.
 * Stopping one started on no task asks which project — and task, if any — the time went to, by
 * opening `choosing`.
 */
export class TimerClock {
    @inject(Orm) orm!: Orm;
    @inject(Notifications) notifications!: Notifications;

    @state accessor status: TimerStatus = { running: false };
    @state accessor choosing = false;
    @state accessor busy = false;

    /** Read the timer again: another tab may have started or stopped it. */
    async reload(): Promise<void> {
        this.status = await this.orm.call<TimerStatus>("timesheet_timer", "timer_status", [], {});
    }

    /** Start the timer on a task, or on none yet; one running on another task is logged first. */
    async start(task: number | null): Promise<void> {
        const before = this.status.task?.[0] ?? null;
        await this.run("timer_start", { task });
        if (before !== null && before !== task) {
            this.orm.touch("project_task");
            this.orm.touch("timesheet");
        }
    }

    /** Stop it through the assistant, which confirms the time and where it went before logging it. */
    stop(): void {
        this.choosing = true;
    }

    /** What stopping the timer answered: the time logged, discarded, or kept running on a project said. */
    ended(answer: unknown): void {
        this.choosing = false;
        if (typeof answer === "object" && answer !== null && (answer as TimerStatus).running) {
            this.status = answer as TimerStatus;
            return;
        }
        this.status = { running: false };
        if (typeof answer === "object" && answer !== null && "timesheet" in answer) {
            this.notifications.add("success", "Time logged.");
            this.orm.touch("project_task");
            this.orm.touch("project_project");
            this.orm.touch("timesheet");
        }
    }

    /** Whether it runs on the task. */
    runsOn(task: unknown): boolean {
        return this.status.running && this.status.task?.[0] === task;
    }

    private async run(method: string, args: Record<string, unknown>): Promise<TimerStatus | null> {
        this.busy = true;
        try {
            const status = await this.orm.call<TimerStatus>("timesheet_timer", method, [], args);
            this.status = status;
            return status;
        } catch (error) {
            this.notifications.add("danger", error instanceof Error ? error.message : String(error));
            return null;
        } finally {
            this.busy = false;
        }
    }
}

/** `0:07:42`: hours, minutes and seconds since the start. */
export function elapsedText(started: string | undefined, now: number): string {
    const seconds = started === undefined ? 0 : Math.max(0, Math.floor((now - Date.parse(started)) / 1000));
    const pad = (value: number): string => String(value).padStart(2, "0");
    return `${Math.floor(seconds / 3600)}:${pad(Math.floor(seconds / 60) % 60)}:${pad(seconds % 60)}`;
}

/**
 * The timer in the menu, on every page: started from there on no task, stopped from there
 * whatever it runs on. Stopped, the assistant `timesheet_timer_stop` confirms the time — which
 * may be corrected — the project, the task if any, and what was done, before logging it; the
 * time may be discarded instead, or the timer kept running.
 */
export class TimerTray extends Component {
    static template = "timesheet.TimerTray";
    static components = { FormDialog };

    props = props({ wide: t.boolean().default(true) });

    @inject(TimerClock) clock!: TimerClock;

    @state accessor now = Date.now();

    @effect loadOnce(): () => void {
        void this.clock.reload();
        const reload = (): void => {
            if (document.visibilityState === "visible") {
                void this.clock.reload();
            }
        };
        document.addEventListener("visibilitychange", reload);
        return () => document.removeEventListener("visibilitychange", reload);
    }

    @effect tick(): (() => void) | void {
        if (!this.clock.status.running) {
            return;
        }
        this.now = Date.now();
        const timer = setInterval(() => (this.now = Date.now()), 1000);
        return () => clearInterval(timer);
    }

    get elapsed(): string {
        return elapsedText(this.clock.status.started, this.now);
    }

    /** What the timer runs on, for its title. */
    get on(): string {
        return this.clock.status.task?.[1] ?? this.clock.status.project?.[1] ?? "No task yet";
    }

    /** The assistant logged the time, discarded it, or kept it running where it was said to go. */
    readonly done = (answer: unknown): void => {
        this.clock.ended(answer);
    };

    readonly close = (): void => {
        this.clock.choosing = false;
    };
}

/**
 * Start or stop the user's timer on the task shown: in the task form. It follows the timer
 * wherever it was started or stopped.
 */
export class TimerWidget extends Widget {
    static override template = "timesheet.TimerWidget";

    override props = props({ ...widgetProps });

    @inject(TimerClock) clock!: TimerClock;

    @state accessor now = Date.now();

    @effect tick(): (() => void) | void {
        if (!this.runs) {
            return;
        }
        this.now = Date.now();
        const timer = setInterval(() => (this.now = Date.now()), 1000);
        return () => clearInterval(timer);
    }

    get task(): number | null {
        const id = this.props.record.id;
        return typeof id === "number" ? id : null;
    }

    get runs(): boolean {
        return this.clock.runsOn(this.task);
    }

    get elapsed(): string {
        return elapsedText(this.clock.status.started, this.now);
    }

    toggle(ev: Event): void {
        ev.stopPropagation();
        if (this.runs) {
            this.clock.stop();
        } else if (this.task !== null) {
            void this.clock.start(this.task);
        }
    }
}

services.add("timesheet.timer", TimerClock);
systray.add("timesheet.timer", TimerTray);
widgets.add("timer", TimerWidget);
