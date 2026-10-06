import { namesRead } from "@web/core/expression";
import type { Column } from "@web/views/view";

/** What a form's XML becomes: a Trame template, and the columns and texts it refers to by index. */
export interface CompiledForm {
    source: string;
    columns: Column[];
    texts: string[];
    buttons: FormButton[];
    /** The fields its conditions read, which the form reads with the record. */
    conditionNames: string[];
    /** Whether it has a `<leader>`, which shrinks once the page is scrolled down. */
    hasLeader: boolean;
    /** The one2many and many2many its related links show, whose records it names, with their action. */
    relatedFields: { name: string; action: string }[];
}

export interface FormButton {
    name: string;
    type: "method" | "action";
}

function escape(text: string): string {
    return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

/** Shown when its own condition does not hide it, and something in it shows. */
function both(own: string, inner: string): string {
    if (own === "true") {
        return inner;
    }
    return inner === "true" ? own : `(${own}) && (${inner})`;
}

function anyOf(conditions: string[]): string {
    if (conditions.length === 0 || conditions.includes("true")) {
        return "true";
    }
    return conditions.map((condition) => `(${condition})`).join(" || ");
}

function shownUnless(element: Element): string {
    const invisible = element.getAttribute("invisible");
    return invisible === null ? "true" : `!(${invisible})`;
}

interface Piece {
    xml: string;
    /** When it shows, as an expression: `true` when always. */
    shown: string;
}

/**
 * Turn a form's XML into the Trame template of its body.
 *
 * Every condition stays the expression the view wrote — `invisible` a `t-if`, `readonly` a prop —
 * evaluated by Trame with each field it reads set to the record's value. A field shown as a
 * `statusbar` goes to the bar at the top, wherever the view put it; `<totals>` sums amounts up
 * under what they sum. A block, a page or pages
 * with nothing shown in them are hidden; the first page shown is open unless the user opened
 * another. What `<side>` and `<chatter/>` hold goes in a column beside the rest; without them, or
 * with nothing shown in them, the rest takes the whole width.
 *
 * A `<leader>` makes a dark band at the top, the record at a glance: its `<actions>` — buttons —
 * and the form's own beside its `role="status"` fields as pills; an `avatar`, a `title` and a
 * `subtitle`; a `figure` — a big amount — with `note`s under it; any other field as a tile. All
 * are shown, not edited: the form below edits them. With a leader, a statusbar stays where the view
 * put it. `<related>` follows: a `<link>` per one2many or many2many, with how many records it
 * holds, opening them under its `action` — once the record exists.
 */
export function compileForm(root: Element, columnOf: (element: Element) => Column): CompiledForm {
    const leader = Array.from(root.children).find((element) => element.tagName === "leader") ?? null;
    const columns: Column[] = [];
    const texts: string[] = [];
    const buttons: FormButton[] = [];
    const conditions: string[] = [];
    let pagesCount = 0;

    const text = (value: string): string => {
        texts.push(value);
        return `{{ __form.label(__form.layout.texts[${texts.length - 1}]) }}`;
    };
    const condition = (element: Element, attribute: string): string | null => {
        const expression = element.getAttribute(attribute);
        if (expression !== null) {
            conditions.push(expression);
        }
        return expression;
    };
    const widget = (element: Element, readonly: string | null): string => {
        columns.push(columnOf(element));
        const column = `__form.layout.columns[${columns.length - 1}]`;
        const editable = readonly === null ? `${column}.field.readonly` : `${column}.field.readonly || !!(${readonly})`;
        return (
            `<t t-component="__form.widgetFor(${column})" record="__form.current" model="__form.props.resModel" name="${column}.name" ` +
            `field="${column}.field" attrs="${column}.attrs" readonly="${escape(editable)}" ` +
            `onChange="__form.changer(${column}.name)" computed="__form.computedLines[${column}.name] ?? {}" ` +
            `computeErrors="__form.lineErrors[${column}.name] ?? {}"/>`
        );
    };
    /**
     * The classes marking a field required, and empty once a save was tried.
     *
     * Required as the view says, or else as its model does: a field that is always set, and not
     * worked out by the server. A check box is left unmarked: it always holds a value.
     */
    const requiredMarks = (element: Element, required: string | null): string => {
        const name = JSON.stringify(element.getAttribute("name") ?? "");
        const { field } = columnOf(element);
        const byModel = field.required && !field.readonly && field.type !== "bool";
        const isRequired = required === null ? String(byModel) : `!!(${required})`;
        return `o_form_required: ${isRequired}, o_form_missing: __form.tried && ${isRequired} && __form.isBlank(${name})`;
    };
    const ifShown = (shown: string): string => (shown === "true" ? "" : ` t-if="${escape(shown)}"`);
    /** A mark beside a field the server could not compute, saying why when hovered. */
    const computeMark = (element: Element): string => {
        const error = escape(`__form.computeErrors[${JSON.stringify(element.getAttribute("name") ?? "")}]`);
        return `<span t-if="${error}" class="o_compute_error" role="img" aria-label="Could not be computed" t-att-title="${error}">!</span>`;
    };

    /** The steps of the record, shown in the bar at the top rather than where the view put them. */
    const statusbars: string[] = [];

    const field = (element: Element): Piece => {
        if (element.getAttribute("widget") === "statusbar" && leader === null) {
            condition(element, "invisible");
            const readonly = condition(element, "readonly");
            statusbars.push(`<div class="o_form_status"${ifShown(shownUnless(element))}>${widget(element, readonly)}</div>`);
            return { xml: "", shown: "false" };
        }
        condition(element, "invisible");
        const readonly = condition(element, "readonly");
        const required = condition(element, "required");
        const shown = shownUnless(element);
        const classes = `{ o_form_field: true, o_form_full: true, ${requiredMarks(element, required)} }`;
        const label =
            element.getAttribute("nolabel") === "1"
                ? computeMark(element)
                : `<span class="o_form_label">{{ __form.label(__form.layout.columns[${columns.length}].label) }}${computeMark(element)}</span>`;
        const xml = `<div t-att-class="${escape(classes)}"${ifShown(shown)}>${label}${widget(element, readonly)}</div>`;
        return { xml, shown };
    };

    const heading = (element: Element): Piece => {
        condition(element, "invisible");
        const shown = shownUnless(element);
        const level = Number(element.tagName.slice(1));
        const parts = Array.from(element.childNodes).map((node) => {
            if (node instanceof Element && node.tagName === "field") {
                const required = condition(node, "required");
                const marks = requiredMarks(node, required);
                return `<span t-att-class="${escape(`{ o_form_heading_field: true, ${marks} }`)}">${widget(node, condition(node, "readonly"))}</span>`;
            }
            const value = node.textContent ?? "";
            if (!value.trim()) {
                return "";
            }
            texts.push(value);
            return `<span>{{ __form.layout.texts[${texts.length - 1}] }}</span>`;
        });
        const xml =
            `<div role="heading" aria-level="${level}" class="o_form_heading o_form_h${level} o_form_full"${ifShown(shown)}>` +
            `${parts.join("")}</div>`;
        return { xml, shown };
    };

    /**
     * Amounts summed up under what they sum, aligned on the right: each field a line, the last
     * one, the total, set apart.
     */
    const totals = (element: Element): Piece => {
        condition(element, "invisible");
        const lines = Array.from(element.children)
            .filter((child) => child.tagName === "field")
            .map((child, at, all) => {
                condition(child, "invisible");
                const shown = shownUnless(child);
                const label = `{{ __form.label(__form.layout.columns[${columns.length}].label) }}`;
                const main = at === all.length - 1 ? " o_form_total_main" : "";
                const xml =
                    `<div class="o_form_total${main}"${ifShown(shown)}><span class="o_form_total_label">${label}</span>` +
                    `<span class="o_form_total_value">${widget(child, "true")}</span></div>`;
                return { xml, shown };
            });
        const shown = both(shownUnless(element), anyOf(lines.map((line) => line.shown)));
        const xml = `<div class="o_form_totals o_form_full"${ifShown(shown)}>${lines.map((line) => line.xml).join("")}</div>`;
        return { xml, shown };
    };

    const block = (element: Element, asCard: boolean): Piece => {
        condition(element, "invisible");
        const inner = contents(element, false);
        const shown = both(shownUnless(element), anyOf(inner.map((piece) => piece.shown)));
        const title = element.getAttribute("string");
        const xml =
            `<section class="${asCard ? "o_form_card" : "o_form_section"}"${ifShown(shown)}>` +
            (title === null ? "" : `<h2 class="o_form_block_title">${text(title)}</h2>`) +
            `<div class="o_form_grid">${inner.map((piece) => piece.xml).join("")}</div></section>`;
        return { xml, shown };
    };

    /**
     * How many lines a page's first one2many or many2many holds, beside its tab: what the user
     * would open it for. Nothing for an empty one, or a page holding none.
     */
    const tabCount = (page: Element): string => {
        const list = Array.from(page.children).find(
            (child) => child.tagName === "field" && columnOf(child).field.type === "refs",
        );
        if (list === undefined) {
            return "";
        }
        const name = escape(JSON.stringify(list.getAttribute("name") ?? ""));
        return `<span t-if="__form.lineCount(${name})" class="o_form_tab_count">{{ __form.lineCount(${name}) }}</span>`;
    };

    const pages = (element: Element): Piece => {
        condition(element, "invisible");
        const key = pagesCount++;
        const open = `open_${key}`;
        const list = Array.from(element.children).map((page) => {
            condition(page, "invisible");
            const inner = contents(page, false);
            const shown = both(shownUnless(page), anyOf(inner.map((piece) => piece.shown)));
            return { page, inner, shown };
        });
        const shown = both(shownUnless(element), anyOf(list.map((entry) => entry.shown)));
        const flags = `[${list.map((entry) => `!!(${entry.shown})`).join(", ")}]`;
        const tabs = list
            .map(
                (entry, at) =>
                    `<button type="button" role="tab"${ifShown(entry.shown)} ` +
                    `t-att-aria-selected="${open} === ${at} ? 'true' : 'false'" ` +
                    `t-att-class="{ o_form_tab: true, active: ${open} === ${at} }" ` +
                    `t-on-click="() => __form.openPage(${key}, ${at})">` +
                    `${text(entry.page.getAttribute("string") ?? entry.page.getAttribute("name") ?? "")}` +
                    `${tabCount(entry.page)}</button>`,
            )
            .join("");
        const panels = list
            .map(
                (entry, at) =>
                    `<div t-if="${open} === ${at}" role="tabpanel" class="o_form_page o_form_grid">` +
                    `${entry.inner.map((piece) => piece.xml).join("")}</div>`,
            )
            .join("");
        const xml =
            `<div class="o_form_pages o_form_full"${ifShown(shown)}>` +
            `<t t-set="${open}" t-value="${escape(`__form.shownPage(${key}, ${flags})`)}"/>` +
            `<div role="tablist" class="o_form_tabs">${tabs}</div>${panels}</div>`;
        return { xml, shown };
    };

    /** What elements show; blocks as cards at the root and in the side column. */
    const contents = (parent: Element, asCards: boolean, children = Array.from(parent.children)): Piece[] =>
        children.flatMap((element): Piece[] => {
            const tag = element.tagName;
            if (tag === "chatter") {
                return [chatter(element)];
            }
            if (tag === "block") {
                return [block(element, asCards)];
            }
            if (tag === "field") {
                return [field(element)];
            }
            if (/^h[1-6]$/.test(tag)) {
                return [heading(element)];
            }
            if (tag === "pages") {
                return [pages(element)];
            }
            if (tag === "totals") {
                return [totals(element)];
            }
            return [];
        });

    /** The record's thread, as the plugin registered under `chatter` in `formParts` shows it. */
    const chatter = (element: Element): Piece => {
        condition(element, "invisible");
        const shown = shownUnless(element);
        const xml =
            `<section class="o_form_card o_form_chatter"${ifShown(shown)}>` +
            `<t t-if="__form.chatter" t-component="__form.chatter" model="__form.props.resModel" ` +
            `record="__form.props.resId ?? null" version="__form.record"/>` +
            `<t t-else=""><h2 class="o_form_block_title">Activity</h2>` +
            `<p class="o_form_chatter_empty">Messages and activities will show here.</p></t></section>`;
        return { xml, shown };
    };

    /**
     * The side column: what every `<side>` holds, and a `<chatter/>` written at the root. Blocks
     * in it are cards, as at the root.
     */
    const side = (): Piece[] =>
        Array.from(root.children).flatMap((element): Piece[] => {
            if (element.tagName === "chatter") {
                return [chatter(element)];
            }
            if (element.tagName !== "side") {
                return [];
            }
            condition(element, "invisible");
            const inner = contents(element, true);
            const shown = both(shownUnless(element), anyOf(inner.map((piece) => piece.shown)));
            return [{ xml: `<t${ifShown(shown)}>${inner.map((piece) => piece.xml).join("")}</t>`, shown }];
        });

    /** Buttons running a method of the record or opening an action, by their place in the form. */
    const buttonsOf = (holders: Element[]): string =>
        holders
            .flatMap((element) => Array.from(element.children))
            .map((button) => {
                condition(button, "invisible");
                buttons.push({
                    name: button.getAttribute("name") ?? "",
                    type: button.getAttribute("type") === "action" ? "action" : "method",
                });
                const at = buttons.length - 1;
                return (
                    `<button type="button"${ifShown(shownUnless(button))} ` +
                    `t-att-class="{ ${button.getAttribute("highlight") === "1" ? "o_button_primary" : "o_button_secondary"}: true, o_button_busy: __form.pressing === ${at} }" ` +
                    `t-att-disabled="__form.busy" t-on-click="() => __form.press(__form.layout.buttons[${at}], ${at})">` +
                    `${text(button.getAttribute("string") ?? button.getAttribute("name") ?? "")}</button>`
                );
            })
            .join("");

    /** A field shown and not edited: its column read with the record, by its position. */
    const shownColumn = (element: Element): string => {
        columns.push(columnOf(element));
        return `__form.layout.columns[${columns.length - 1}]`;
    };
    const fieldName = (element: Element): string => escape(JSON.stringify(element.getAttribute("name") ?? ""));
    const rolesIn = (element: Element | null, role: string | null): Element[] =>
        Array.from(element?.children ?? []).filter(
            (child) => child.tagName === "field" && child.getAttribute("role") === role,
        );

    const leaderXml = (controls: string): string => {
        if (leader === null) {
            return "";
        }
        const shownIf = (element: Element): string => {
            condition(element, "invisible");
            return ifShown(shownUnless(element));
        };
        const actions = buttonsOf(Array.from(leader.children).filter((element) => element.tagName === "actions"));
        const pills = rolesIn(leader, "status")
            .map((element) => {
                shownColumn(element);
                return `<span class="o_leader_status"${shownIf(element)}>{{ __form.display(${fieldName(element)}) }}</span>`;
            })
            .join("");
        const avatar = rolesIn(leader, "avatar")
            .slice(0, 1)
            .map((element) => {
                shownColumn(element);
                const name = fieldName(element);
                return (
                    `<t t-if="__form.initials(${name})"><span class="o_leader_avatar" aria-hidden="true" ` +
                    `t-att-style="__form.avatarStyle(${name})"${shownIf(element)}>{{ __form.initials(${name}) }}</span></t>`
                );
            })
            .join("");
        const title = rolesIn(leader, "title")
            .slice(0, 1)
            .map((element) => {
                shownColumn(element);
                return `<h2 class="o_leader_title">{{ __form.display(${fieldName(element)}) || __form.title }}</h2>`;
            })
            .join("");
        const subtitles = rolesIn(leader, "subtitle")
            .map((element) => {
                shownColumn(element);
                return `<span class="o_leader_subtitle"${shownIf(element)}>{{ __form.display(${fieldName(element)}) }}</span>`;
            })
            .join("");
        const figure = rolesIn(leader, "figure")
            .slice(0, 1)
            .map((element) => {
                const label = `{{ __form.label(__form.layout.columns[${columns.length}].label) }}`;
                return (
                    `<span class="o_leader_figure_label">${label}</span>` +
                    `<span class="o_leader_figure_value">${widget(element, "true")}</span>`
                );
            })
            .join("");
        const notes = rolesIn(leader, "note")
            .map((element) => {
                const shown = shownIf(element);
                const label = `{{ __form.label(__form.layout.columns[${columns.length}].label) }}`;
                return `<span class="o_leader_note"${shown}>${label} ${widget(element, "true")}</span>`;
            })
            .join("");
        const tiles = rolesIn(leader, null)
            .map((element) => {
                const shown = shownIf(element);
                const label = `{{ __form.label(__form.layout.columns[${columns.length}].label) }}`;
                return (
                    `<div class="o_leader_tile"${shown}><span class="o_leader_tile_label">${label}</span>` +
                    `<span class="o_leader_tile_value">${widget(element, "true")}</span></div>`
                );
            })
            .join("");
        return (
            `<header t-att-class="{ o_leader: true, o_leader_compact: __form.leaderCompact }">` +
            `<div class="o_leader_top">${actions}${pills}<span class="o_form_bar_gap"/>${controls}</div>` +
            `<div class="o_leader_identity">${avatar}<div class="o_leader_names">${title}${subtitles}</div>` +
            (figure || notes ? `<div class="o_leader_figure">${figure}${notes}</div>` : "") +
            `</div>` +
            (tiles ? `<div class="o_leader_tiles">${tiles}</div>` : "") +
            `</header>`
        );
    };

    /** The records the form's one2many and many2many hold, each a link opening them. */
    const relatedFields: { name: string; action: string }[] = [];
    const relatedXml = (): string => {
        const links = Array.from(root.children)
            .filter((element) => element.tagName === "related")
            .flatMap((element) => {
                condition(element, "invisible");
                return Array.from(element.children).filter((child) => child.tagName === "link");
            });
        if (links.length === 0) {
            return "";
        }
        const items = links
            .map((link, at) => {
                condition(link, "invisible");
                const column = shownColumn(link);
                relatedFields.push({ name: link.getAttribute("name") ?? "", action: link.getAttribute("action") ?? "" });
                const name = fieldName(link);
                const action = escape(JSON.stringify(link.getAttribute("action") ?? ""));
                const label = link.getAttribute("string");
                const caption = label === null ? `{{ __form.label(${column}.label) }}` : text(label);
                const icon = escape(JSON.stringify(link.getAttribute("icon") ?? ""));
                return (
                    `<div class="o_related_item"${ifShown(shownUnless(link))}>` +
                    `<button type="button" t-att-class="{ o_related_link: true, o_related_none: !__form.relatedRecords(${name}).length }" ` +
                    `t-att-aria-expanded="__form.openLink === ${at} ? 'true' : 'false'" ` +
                    `t-on-click="() => __form.followLink(${at}, ${action}, ${name})">` +
                    `<span class="o_related_icon"><Icon name="${icon}"/></span>` +
                    `<span class="o_related_text"><span class="o_related_head">` +
                    `<span class="o_related_count">{{ __form.relatedRecords(${name}).length }}</span>` +
                    `<span class="o_related_label">${caption}</span></span>` +
                    `<span class="o_related_sub">{{ __form.relatedSummary(${name}) }}</span></span></button>` +
                    `<ul t-if="__form.openLink === ${at}" class="o_related_menu">` +
                    `<li t-foreach="__form.relatedRecords(${name})" t-as="linked" t-key="linked.id">` +
                    `<button type="button" t-on-click="() => __form.openRelated(${action}, linked.id)">{{ linked.name }}</button>` +
                    `</li></ul></div>`
                );
            })
            .join("");
        const shown = anyOf(
            Array.from(root.children)
                .filter((element) => element.tagName === "related")
                .map(shownUnless),
        );
        return `<nav class="o_related" aria-label="Related documents"${ifShown(`!__form.isNew && (${shown})`)}>${items}</nav>`;
    };

    const bar = buttonsOf(Array.from(root.children).filter((element) => element.tagName === "buttons"));
    const main = contents(
        root,
        true,
        Array.from(root.children).filter((element) => element.tagName !== "chatter"),
    )
        .map((piece) => piece.xml)
        .join("");
    const aside = side();
    const sideShown = aside.length === 0 ? "false" : anyOf(aside.map((piece) => piece.shown));
    const body =
        `<div t-att-class="${escape(`{ o_form_layout: true, o_form_with_side: ${sideShown} }`)}">` +
        `<div class="o_form_main">${main}</div>` +
        (aside.length === 0
            ? ""
            : `<aside class="o_form_side"${ifShown(sideShown)}>${aside.map((piece) => piece.xml).join("")}</aside>`) +
        `</div>`;

    const controls =
        `<div t-if="__form.pager" class="o_pager"><span class="o_pager_value">` +
        `{{ __form.pager.position }} / {{ __form.pager.total }}</span>` +
        `<button type="button" class="o_pager_button" aria-label="Previous record" ` +
        `t-att-disabled="__form.busy || __form.pager.previous === null" t-on-click="() => __form.step(__form.pager.previous)">` +
        `<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" ` +
        `stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m15 18-6-6 6-6"/></svg></button>` +
        `<button type="button" class="o_pager_button" aria-label="Next record" ` +
        `t-att-disabled="__form.busy || __form.pager.next === null" t-on-click="() => __form.step(__form.pager.next)">` +
        `<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" ` +
        `stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m9 18 6-6-6-6"/></svg></button></div>` +
        `<span t-if="__form.isDirty" class="o_form_dirty" role="status">Unsaved changes</span>` +
        `<button t-if="__form.isDirty" type="button" class="o_button_secondary" t-att-disabled="__form.busy" ` +
        `t-on-click="() => __form.discard()">Discard</button>` +
        `<button t-if="__form.isDirty || __form.isNew" type="button" ` +
        `t-att-class="{ o_button_primary: true, o_button_busy: __form.saving }" t-att-disabled="!__form.canSave" ` +
        `t-on-click="() => __form.save()">{{ __form.saving ? "Saving…" : "Save" }}</button>`;
    const top =
        leader === null
            ? `<div class="o_form_bar">${bar}<span class="o_form_bar_gap"/>${statusbars.join("")}${controls}</div>`
            : leaderXml(controls);
    const related = relatedXml();

    const conditionNames = [...new Set(conditions.flatMap(namesRead))];
    const values = conditionNames
        .map((name) => `<t t-set="${name}" t-value="${escape(`__form.conditionValue(${JSON.stringify(name)})`)}"/>`)
        .join("");
    const source =
        `<div class="o_form_body" t-ref="__form.element">${values}${top}` +
        `<p t-if="__form.failure" class="o_form_failure" role="alert">{{ __form.failure }}</p>` +
        `${related}${body}</div>`;
    return { source, columns, texts, buttons, conditionNames, hasLeader: leader !== null, relatedFields };
}
