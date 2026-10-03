import type { Column } from "@web/views/view";

/** What a form's XML becomes: a Trame template, and the columns and texts it refers to by index. */
export interface CompiledForm {
    source: string;
    columns: Column[];
    texts: string[];
    buttons: FormButton[];
    /** The fields its conditions read, which the form reads with the record. */
    conditionNames: string[];
}

export interface FormButton {
    name: string;
    type: "method" | "action";
}

/** Names an expression may read without being fields: the language's own, and a few globals. */
const EXPRESSION_WORDS = new Set([
    "true", "false", "null", "undefined", "typeof", "instanceof", "in", "of", "new", "void",
    "NaN", "Infinity", "Math", "Number", "String", "Boolean", "Array", "Date", "JSON", "Object",
]);

/**
 * The names a condition reads: what is left once strings, numbers, properties (`.includes`), the
 * language's words and the parameters of arrow functions (`x => x.id`) are set aside. The server
 * reads them the same way, to refuse one its model lacks.
 */
export function namesRead(expression: string): string[] {
    const parameters = new Set<string>();
    for (const match of expression.matchAll(/(?:\(([^()]*)\)|([A-Za-z_$][\w$]*))\s*=>/g)) {
        for (const name of (match[1] ?? match[2]).split(",")) {
            if (name.trim()) {
                parameters.add(name.trim());
            }
        }
    }
    const withoutStrings = expression.replace(/'(?:\\.|[^'\\])*'|"(?:\\.|[^"\\])*"|`(?:\\.|[^`\\])*`/g, " ");
    const names: string[] = [];
    for (const match of withoutStrings.matchAll(/(\.\s*)?([A-Za-z_$][\w$]*)/g)) {
        const [, property, name] = match;
        const before = withoutStrings[match.index - 1];
        if (property || (before !== undefined && /\d/.test(before)) || EXPRESSION_WORDS.has(name) || parameters.has(name)) {
            continue;
        }
        if (!names.includes(name)) {
            names.push(name);
        }
    }
    return names;
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
 * evaluated by Trame with each field it reads set to the record's value. A block, a page or pages
 * with nothing shown in them are hidden; the first page shown is open unless the user opened
 * another.
 */
export function compileForm(root: Element, columnOf: (element: Element) => Column): CompiledForm {
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
            `<t t-component="__form.widgetFor(${column})" record="__form.current" name="${column}.name" ` +
            `field="${column}.field" attrs="${column}.attrs" readonly="${escape(editable)}" ` +
            `onChange="__form.changer(${column}.name)"/>`
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

    const field = (element: Element): Piece => {
        condition(element, "invisible");
        const readonly = condition(element, "readonly");
        const required = condition(element, "required");
        const shown = shownUnless(element);
        const classes = `{ o_form_field: true, o_form_full: true, ${requiredMarks(element, required)} }`;
        const label =
            element.getAttribute("nolabel") === "1"
                ? ""
                : `<span class="o_form_label">{{ __form.label(__form.layout.columns[${columns.length}].label) }}</span>`;
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

    const block = (element: Element, isRoot: boolean): Piece => {
        condition(element, "invisible");
        const inner = contents(element, false);
        const shown = both(shownUnless(element), anyOf(inner.map((piece) => piece.shown)));
        const title = element.getAttribute("string");
        const xml =
            `<section class="${isRoot ? "o_form_card" : "o_form_section"}"${ifShown(shown)}>` +
            (title === null ? "" : `<h2 class="o_form_block_title">${text(title)}</h2>`) +
            `<div class="o_form_grid">${inner.map((piece) => piece.xml).join("")}</div></section>`;
        return { xml, shown };
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
                    `${text(entry.page.getAttribute("string") ?? entry.page.getAttribute("name") ?? "")}</button>`,
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

    const contents = (parent: Element, isRoot: boolean): Piece[] =>
        Array.from(parent.children).flatMap((element): Piece[] => {
            const tag = element.tagName;
            if (tag === "block") {
                return [block(element, isRoot)];
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
            return [];
        });

    const bar = Array.from(root.children)
        .filter((element) => element.tagName === "buttons")
        .flatMap((element) => Array.from(element.children))
        .map((button) => {
            condition(button, "invisible");
            buttons.push({
                name: button.getAttribute("name") ?? "",
                type: button.getAttribute("type") === "action" ? "action" : "method",
            });
            const at = buttons.length - 1;
            return (
                `<button type="button" class="o_button_secondary"${ifShown(shownUnless(button))} ` +
                `t-att-disabled="__form.saving" t-on-click="() => __form.press(__form.layout.buttons[${at}])">` +
                `${text(button.getAttribute("string") ?? button.getAttribute("name") ?? "")}</button>`
            );
        })
        .join("");
    const body = contents(root, true)
        .map((piece) => piece.xml)
        .join("");

    const conditionNames = [...new Set(conditions.flatMap(namesRead))];
    const values = conditionNames
        .map((name) => `<t t-set="${name}" t-value="${escape(`__form.conditionValue(${JSON.stringify(name)})`)}"/>`)
        .join("");
    const source =
        `<div class="o_form_body" t-ref="__form.element">${values}` +
        `<div class="o_form_bar">${bar}<span class="o_form_bar_gap"/>` +
        `<button t-if="__form.isDirty" type="button" class="o_button_secondary" t-att-disabled="__form.saving" ` +
        `t-on-click="() => __form.discard()">Discard</button>` +
        `<button type="button" class="o_button_primary" t-att-disabled="!__form.canSave" ` +
        `t-on-click="() => __form.save()">Save</button></div>` +
        `<p t-if="__form.failure" class="o_form_failure" role="alert">{{ __form.failure }}</p>` +
        `${body}</div>`;
    return { source, columns, texts, buttons, conditionNames };
}
