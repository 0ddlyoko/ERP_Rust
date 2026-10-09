import { mount, registerTemplates } from "trame";
import { Menus } from "./core/menus";
import { Breadcrumb } from "./core/breadcrumb";
import { installDialogDrag } from "./core/dialog_drag";
import { Models } from "./core/models";
import { Notifications } from "./core/notifications";
import { Orm } from "./core/orm";
import { RecordColors } from "./core/record_colors";
import { Router } from "./core/router";
import { Rpc } from "./core/rpc";
import { Session } from "./core/session";
import { Views } from "./core/views";
import { services } from "./web_client/systray";
import { WebClient } from "./web_client/web_client";

/**
 * Register the back office's templates, then mount the web client where the page leaves room,
 * with the services every component may inject — plugins' among them. An error out of every boundary while it first
 * renders is shown in the page, rather than left as a blank one.
 */
async function start(): Promise<void> {
    const target = document.querySelector(".o_web_client_root");
    if (target === null) {
        return;
    }
    installDialogDrag();
    const response = await fetch("/web/assets/web.assets_backend.xml");
    registerTemplates(await response.text(), "web.assets_backend.xml");
    try {
        await mount(WebClient, target, { provide: [Session.fromPage(), Router, Breadcrumb, Notifications, Rpc, Orm, Models, Views, Menus, RecordColors, ...services.getAll()] });
    } catch (error) {
        console.error(error);
        const failure = document.createElement("p");
        failure.className = "o_view_error";
        failure.textContent = `The web client could not start: ${error instanceof Error ? error.message : String(error)}`;
        target.replaceChildren(failure);
    }
}

void start();
