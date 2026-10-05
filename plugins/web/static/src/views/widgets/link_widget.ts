import { props } from "trame";
import { Widget, widgetProps, widgets } from "./widget";

/**
 * Text that leads somewhere: an address to write to, a number to call, a site to visit. Typed in
 * an input of its kind, with a button beside it following the link once there is one; shown as
 * a link, which opens without opening the record a list shows it in.
 */
abstract class LinkWidget extends Widget {
    static override template = "web.LinkWidget";

    override props = props({ ...widgetProps });

    /** Where the link leads. */
    abstract get href(): string;

    /** What following the link does, said by the button beside the input. */
    abstract get label(): string;

    /** The path of the button's icon, in a 24 by 24 box. */
    abstract get icon(): string;

    /** Whether the link opens in a new tab rather than leaving the page. */
    get external(): boolean {
        return false;
    }
}

/** An email address, written to by its link. */
export class EmailWidget extends LinkWidget {
    override get inputType(): string {
        return "email";
    }

    get href(): string {
        return `mailto:${this.text}`;
    }

    get label(): string {
        return "Send an email";
    }

    get icon(): string {
        return "M4 6h16v12H4z M4 7l8 6 8-6";
    }
}

/** A phone number, called by its link. */
export class PhoneWidget extends LinkWidget {
    override get inputType(): string {
        return "tel";
    }

    get href(): string {
        return `tel:${this.text.replace(/\s+/g, "")}`;
    }

    get label(): string {
        return "Call";
    }

    get icon(): string {
        return "M5 4h4l2 5-2.5 1.5a11 11 0 0 0 5 5L15 13l5 2v4a2 2 0 0 1-2 2A16 16 0 0 1 3 6a2 2 0 0 1 2-2";
    }
}

/** A website, opened in a new tab; `https://` when the address says no scheme. */
export class UrlWidget extends LinkWidget {
    override get inputType(): string {
        return "url";
    }

    get href(): string {
        return /^[a-z][a-z0-9+.-]*:/i.test(this.text) ? this.text : `https://${this.text}`;
    }

    get label(): string {
        return "Open the website";
    }

    get icon(): string {
        return "M14 4h6v6 M20 4 10 14 M19 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1h5";
    }

    override get external(): boolean {
        return true;
    }
}

widgets.add("email", EmailWidget);
widgets.add("phone", PhoneWidget);
widgets.add("url", UrlWidget);
