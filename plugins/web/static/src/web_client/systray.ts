import { type ComponentClass, registry } from "trame";

/**
 * What plugins show at the bottom of the menu, on every page: components given `wide`, whether
 * the menu shows names or icons only.
 */
export const systray = registry.category<ComponentClass>("systray");

/** Services plugins add to those every component may inject: classes, or instances. */
export const services = registry.category<unknown>("services");
