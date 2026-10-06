import { Component, type ComponentClass, props, registerTemplate, t } from "trame";
import { Icon } from "@web/core/icons";
import type { FormView } from "@web/views/form/form_view";

/**
 * What the template of a form's body reads: the form, under a name no field can hide.
 *
 * The form itself rather than a read-only view of it: the body is its own layout, and edits it.
 */
export class FormBody extends Component {
    static components = { Icon };

    props = props({ form: t.any<FormView>() });

    get __form(): FormView {
        return this.props.form as FormView;
    }
}

const bodies = new Map<string, ComponentClass>();

/** The component showing a form's body as this template says: one per template, made once. */
export function bodyFor(source: string): ComponentClass {
    let body = bodies.get(source);
    if (body === undefined) {
        const name = `web.FormBody.${bodies.size}`;
        registerTemplate(name, source);
        body = class extends FormBody {
            static template = name;
        };
        bodies.set(source, body);
    }
    return body;
}
