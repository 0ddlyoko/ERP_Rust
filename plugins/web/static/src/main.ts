import { mount, registerTemplates } from "trame";
import { WebClient } from "./web_client/web_client";

/** Register the back office's templates, then mount the web client where the page leaves room. */
async function start(): Promise<void> {
    const response = await fetch("/web/assets/web.assets_backend.xml");
    registerTemplates(await response.text(), "web.assets_backend.xml");
    const target = document.querySelector(".o_web_client_root");
    if (target !== null) {
        await mount(WebClient, target);
    }
}

void start();
