import { Component, props, t } from "trame";
import icons from "lucide";

/**
 * The strokes of an icon data names — a menu's `icon="shopping-cart"` — among Lucide's
 * (https://lucide.dev/icons): data names an icon rather than drawing it, so that every icon shares
 * one stroke and one size. A name Lucide does not know is drawn as a dot. A stroke starting with
 * `!` is a shape filled rather than outlined.
 */
export function iconPaths(name: string | null | undefined): readonly string[] {
    return (name ? icons[name] : undefined) ?? icons.dot;
}

/** An icon by its name, in the colour of the text around it. */
export class Icon extends Component {
    static template = "web.Icon";

    props = props({
        name: t.string().orNull().default(null),
        size: t.number().default(19),
    });

    get strokes(): readonly string[] {
        return iconPaths(this.props.name);
    }
}
