import { type ComponentClass, Component, props, t } from "trame";
import { viewKinds } from "@web/views/view";

/**
 * A form creating a record of a model, over the view the user is on: the address and the
 * breadcrumb stay where they were. `onCreated` gets the record once saved, as `[id, name]`.
 *
 * As a `wizard`, the form is an assistant: its own heading and the buttons of its `<footer>`,
 * whose method's answer `onDone` gets.
 */
export class FormDialog extends Component {
    static template = "web.FormDialog";

    props = props({
        model: t.string(),
        defaults: t.object().default({}),
        onCreated: t.func<(record: [number, string | null]) => void>().optional(),
        onClose: t.func<() => void>(),
        wizard: t.boolean().default(false),
        onDone: t.func<(answer: unknown) => void>().optional(),
    });

    /** The form view, as registered: one a plugin put in its place is the one shown. */
    get form(): ComponentClass {
        return viewKinds.get("form");
    }

    readonly created = (record: [number, string | null]): void => {
        this.props.onCreated?.(record);
        this.props.onClose();
    };
}
