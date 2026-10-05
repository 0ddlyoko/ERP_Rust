import { props } from "trame";
import { Widget, widgetProps, widgets } from "./widget";

/**
 * A number in the browser's notation: `1 234,5` or `1,234.5`; with `digits="2"`, always with
 * that many decimals.
 */
export class DecimalWidget extends Widget {
    override props = props({ ...widgetProps });

    /** How many decimals are shown, when the view says; as many as the number has otherwise. */
    get digits(): number | undefined {
        const digits = Number(this.props.attrs.digits);
        return Number.isInteger(digits) && digits >= 0 ? digits : undefined;
    }

    override get text(): string {
        if (this.isEmpty) {
            return "";
        }
        const digits = this.digits;
        return digits === undefined
            ? Number(this.value).toLocaleString()
            : Number(this.value).toLocaleString(undefined, { minimumFractionDigits: digits, maximumFractionDigits: digits });
    }

    override get inputType(): string {
        return "number";
    }

    /** Sent as text, as the server reads a decimal: no float rounding on the way. */
    override parse(text: string): unknown {
        return text.trim() === "" ? null : text.trim();
    }
}

widgets.add("decimal", DecimalWidget);

/** An amount of money: two decimals unless the view says otherwise, `1 234,50`. */
export class MonetaryWidget extends DecimalWidget {
    override get digits(): number {
        return super.digits ?? 2;
    }
}

widgets.add("monetary", MonetaryWidget);
