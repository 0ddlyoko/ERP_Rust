import { state } from "trame";
import { services } from "@web/web_client/systray";

/**
 * The message to bring into sight once its record shows: one opened from the inbox. The thread
 * showing it scrolls to it, marks it a moment, and forgets it.
 */
export class MessageFocus {
    @state accessor wanted: { model: string; record: number; message: number } | null = null;
}

services.add("mail.focus", MessageFocus);
