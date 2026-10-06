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

/**
 * An amount of money, in its currency as the browser writes it — `1 234,50 €`, `$1,234.50` —
 * read from the record's `currency` field, or the one `currency_field` names. Two decimals
 * unless the view says otherwise; without a currency, the number alone.
 */
export class MonetaryWidget extends DecimalWidget {
    override get digits(): number {
        return super.digits ?? 2;
    }

    /** The code of the record's currency, `EUR`, when it has one. */
    get currency(): string | null {
        const field = (this.props.attrs as Record<string, string>).currency_field ?? "currency";
        const value = this.props.record[field];
        const code = Array.isArray(value) ? value[1] : null;
        return typeof code === "string" && /^[A-Z]{3}$/.test(code) ? code : null;
    }

    override get text(): string {
        const currency = this.currency;
        if (this.isEmpty || currency === null) {
            return super.text;
        }
        const digits = this.digits;
        return Number(this.value).toLocaleString(undefined, {
            style: "currency",
            currency,
            minimumFractionDigits: digits,
            maximumFractionDigits: digits,
        });
    }
}

/**
 * The fields read besides those shown, for the widgets showing them: the currency of an amount.
 */
export function companionFields(columns: readonly { widget?: string; attrs: Record<string, string> }[], fields: Record<string, unknown>): string[] {
    return columns
        .filter((column) => column.widget === "monetary")
        .map((column) => column.attrs.currency_field ?? "currency")
        .filter((name, at, names) => name in fields && names.indexOf(name) === at);
}

widgets.add("monetary", MonetaryWidget);
