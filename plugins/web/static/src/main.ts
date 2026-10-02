import { mount, registerTemplates } from "trame";
import { Menus } from "./core/menus";
import { Models } from "./core/models";
import { Orm } from "./core/orm";
import { Router } from "./core/router";
import { Rpc } from "./core/rpc";
import { Session } from "./core/session";
import { Views } from "./core/views";
import { WebClient } from "./web_client/web_client";

/**
 * Register the back office's templates, then mount the web client where the page leaves room,
 * with the services every component may inject.
 */
async function start(): Promise<void> {
    const target = document.querySelector(".o_web_client_root");
    if (target === null) {
        return;
    }
    const response = await fetch("/web/assets/web.assets_backend.xml");
    registerTemplates(await response.text(), "web.assets_backend.xml");
    await mount(WebClient, target, { provide: [Session.fromPage(), Router, Rpc, Orm, Models, Views, Menus] });
}

void start();
