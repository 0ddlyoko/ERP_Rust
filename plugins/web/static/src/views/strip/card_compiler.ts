import { type ComponentClass, registerTemplate } from "trame";
import type { Column } from "@web/views/view";
import { CardBody } from "./card_body";

/** A card of a list's records as a template, with the fields it shows by position. */
export interface CompiledCard {
    body: ComponentClass;
    columns: Column[];
}

/** What each tag of a card becomes, and the class it is drawn with. */
const TAGS: Record<string, string> = {
    row: "o_card_row",
    column: "o_card_column",
    title: "o_card_title",
    subtitle: "o_card_subtitle",
    figure: "o_card_figure",
    muted: "o_card_muted",
};

function escape(text: string): string {
    return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

const bodies = new Map<string, ComponentClass>();

/**
 * Turn a card's XML — a list's `<compact>`, `<folded>` or `<preview>` — into the component
 * showing one record as it says.
 *
 * Tags lay the card out, and the theme draws them: `<row>` side by side, `<column>` one under
 * another, `<title>`, `<subtitle>`, `<figure>` — a big number — and `<muted>` for their text,
 * `<spacer/>` pushing what follows to the end of a row. A `<field>` shows its value with its widget,
 * never edited; text between fields is kept. Any of them may be `invisible`, as a condition on the
 * record. A card's template is made once, however many records it shows.
 *
 * Where the card is shown by a view that edits from it — a kanban — a field saying
 * `quick_edit="1"` is changed there: stars or a check box at once, anything else in a small
 * editor opened by a click on it, the rest of the card still opening the record.
 */
export function compileCard(root: Element, columnOf: (element: Element) => Column, quickEdits = false): CompiledCard {
    const columns: Column[] = [];

    const node = (child: ChildNode): string => {
        if (!(child instanceof Element)) {
            return escape(child.textContent ?? "");
        }
        const invisible = child.getAttribute("invisible");
        const shown = invisible === null ? "" : ` t-if="${escape(`!__strip.holds(props.record, ${JSON.stringify(invisible)})`)}"`;
        if (child.tagName === "field") {
            const described = columnOf(child);
            columns.push(described);
            const column = `props.card.columns[${columns.length - 1}]`;
            const widget =
                `<t t-component="__strip.widgetFor(${column})" record="props.record" ` +
                `model="__strip.props.resModel" name="${column}.name" field="${column}.field" attrs="${column}.attrs"`;
            if (quickEdits && child.getAttribute("quick_edit") === "1") {
                const inline = described.widget === "priority" || described.field.type === "bool";
                if (inline) {
                    return (
                        `<span class="o_card_quick"${shown} t-on-click.stop="() => {}">${widget} readonly="false" ` +
                        `onChange="(value) => __strip.quickEdit(props.record, ${column}, value)"/></span>`
                    );
                }
                return (
                    `<button type="button" class="o_card_quick o_card_quick_open"${shown} title="Change" ` +
                    `t-on-click.stop="(ev) => __strip.openQuickEdit(props.record, ${column}, ev)">${widget}/></button>`
                );
            }
            return `<t${shown}>${widget}/></t>`;
        }
        if (child.tagName === "spacer") {
            return `<span class="o_card_spacer"${shown}/>`;
        }
        const className = TAGS[child.tagName];
        if (className === undefined) {
            return "";
        }
        return `<div class="${className}"${shown}>${Array.from(child.childNodes).map(node).join("")}</div>`;
    };

    const source = `<div class="o_card">${Array.from(root.childNodes).map(node).join("")}</div>`;
    let body = bodies.get(source);
    if (body === undefined) {
        const name = `web.CardBody.${bodies.size}`;
        registerTemplate(name, source);
        body = class extends CardBody {
            static template = name;
        };
        bodies.set(source, body);
    }
    return { body, columns };
}
