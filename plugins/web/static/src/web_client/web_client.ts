import { Component } from "trame";
import { ListView } from "../views/list/list_view";

/** The root of the back office: every action is rendered in it. */
export class WebClient extends Component {
    static template = "web.WebClient";
    static components = { ListView };
}
