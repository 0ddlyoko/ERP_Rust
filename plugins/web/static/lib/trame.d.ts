/*! Trame v0.2.3 | LGPL v3 | https://github.com/0ddlyoko/Trame */
// Déclarations de "trame", "trame/testing", "trame/runtime" et "trame/compiler".

declare module "trame/internal/api" {
    /**
     * API publique de Trame (commune à trame.js et trame.runtime.js).
     */
    export { Component, type ComponentClass } from "trame/internal/runtime/component";
    export { mount, type MountOptions, type Root } from "trame/internal/runtime/app";
    export { Suspense, ErrorBoundary, ErrorHandler, Portal } from "trame/internal/runtime/builtins";
    export { xml, Template, registerTemplate, registerTemplates, registerCompiled, getTemplate, extendTemplate, inheritTemplate, type CompiledTemplate, type RenderFactory, } from "trame/internal/runtime/template";
    export { markup, Markup } from "trame/internal/runtime/regions";
    export { state, computed, resource, load, effect, provide, inject } from "trame/internal/decorators";
    export { loading, error, refresh, type ResourceContext, type ResourceOptions } from "trame/internal/reactivity/resource";
    export { batch, untrack, nextTick } from "trame/internal/reactivity/core";
    export { markRaw, toRaw } from "trame/internal/reactivity/store";
    export { props, t, Validator, type PropsOf, type PropsInputOf, type ComponentPropsInput, type DeepReadonly } from "trame/internal/props";
    export { setTranslator, _t } from "trame/internal/i18n";
    export { patch } from "trame/internal/patch";
    export { Registry, registry, type AddOptions } from "trame/internal/registry";
    export const VERSION: string;
}

declare module "trame/internal/compiler/codegen" {
    /**
     * Génération de code : AST → fonction JS de rendu.
     *
     * Pour chaque bloc (le template, une branche de t-if, une ligne de t-foreach, un slot...) :
     * - la partie statique est décrite une fois (`$h.tpl(...)`), construite une seule fois puis clonée ;
     * - le code navigue jusqu'aux nœuds dynamiques (firstChild / nextSibling), puis crée un petit effet
     *   par liaison : seul le nœud concerné est mis à jour quand un signal change.
     *
     * Le code généré n'utilise que de la syntaxe ES2015 (compatibilité navigateurs).
     */
    import { type ExpressionScope } from "trame/internal/compiler/expression";
    import type { AST, Pos } from "trame/internal/compiler/parser";
    export type CompileMode = "component" | "call";
    class Scope implements ExpressionScope {
        private readonly parent;
        private readonly vars;
        private readonly mode;
        /** Appelé quand une variable de cette portée est utilisée. */
        private readonly onUse;
        constructor(parent: Scope | null, vars: Map<string, string>, mode: CompileMode, 
        /** Appelé quand une variable de cette portée est utilisée. */
        onUse?: ((name: string) => void) | null);
        static root(mode: CompileMode): Scope;
        with(name: string, code: string): Scope;
        withAll(entries: [string, string][], onUse?: ((name: string) => void) | null): Scope;
        resolve(name: string): string | undefined;
        free(name: string): string;
    }
    export class CodeGenerator {
        private readonly mode;
        private readonly templateName;
        private readonly statics;
        private readonly locations;
        private counter;
        /** Position du nœud en cours de génération (messages d'erreur de compilation). */
        pos: Pos | undefined;
        constructor(mode: CompileMode, templateName: string);
        uid(prefix: string): string;
        addStatic(code: string): string;
        expr(src: string, scope: Scope): string;
        /** « template "X", ligne N » (avec l'origine si le nœud vient d'une extension). */
        describe(pos: Pos | undefined): string;
        /**
         * Localisation d'une liaison, pour les erreurs d'exécution en mode dev. Déclarée une fois
         * (constante du template) et passée par référence : aucun coût à l'exécution.
         */
        location(pos: Pos | undefined, snippet: string): string;
        generate(ast: AST): string;
    }
    export function generateCode(ast: AST, mode: CompileMode, name: string): string;
}

declare module "trame/internal/compiler/expression" {
    /**
     * Réécriture des expressions JavaScript des templates.
     *
     * Dans un template, on écrit `order.total` (sans `this.`). Le compilateur réécrit chaque identifiant libre :
     * - nom déclaré dans l'expression (paramètre, `const`/`let`/`var`, fonction, `catch`...) → laissé tel quel ;
     * - variable locale du template (t-as, t-set, portée de slot...) → sa variable JS générée ;
     * - global JS connu (Math, JSON...) → laissé tel quel ;
     * - sinon → membre du composant (`$c.order.total`).
     *
     * L'expression est analysée par un vrai parseur (sous-ensemble de JavaScript : expressions, fonctions
     * fléchées ou non, et instructions dans leur corps). Les portées sont donc exactes : une variable
     * déclarée dans le corps d'une fonction n'est pas confondue avec un membre du composant. Le texte
     * d'origine est conservé ; seuls les identifiants concernés sont remplacés.
     *
     * `loading(x)`, `error(x)` et `refresh(x)` sont des macros : l'argument est passé sous forme de
     * fonction, pour être évalué en mode observation (sans déclencher de chargement). Si le composant a une
     * méthode de ce nom, c'est elle qui est appelée (un membre du composant l'emporte toujours).
     */
    export interface ExpressionScope {
        /** Code JS pour une variable locale du template, ou undefined si ce n'est pas une locale. */
        resolve(name: string): string | undefined;
        /** Code JS pour un identifiant libre (membre du composant par défaut). */
        free(name: string): string;
        /**
         * Code d'un appel `loading(args)` / `error(args)` / `refresh(args)` dont le nom n'est pas une
         * variable locale. Par défaut : la méthode du composant si elle existe, sinon la macro.
         */
        macro?(name: string, args: string): string;
    }
    /** Compile une expression de template en code JS. */
    export function compileExpression(src: string, scope: ExpressionScope): string;
    /** Une expression est-elle un simple chemin (a, a.b, a.b.c) ? Utilisé pour les gestionnaires d'événements. */
    export function isSimplePath(src: string): boolean;
    /** Vérifie qu'une chaîne est un identifiant JS valide (pour t-as, t-set...). */
    export function isIdentifier(name: string): boolean;
}

declare module "trame/internal/compiler/files" {
    /**
     * Fichiers de templates (façon Odoo), un ou plusieurs par module :
     *
     *   <templates>
     *       <t t-name="sale.OrderForm">                       nouveau template
     *           <div class="o-order">...</div>
     *       </t>
     *
     *       <t t-inherit="web.Card">                           extension d'un template existant
     *           <xpath expr="//h1" position="after">...</xpath>
     *       </t>
     *
     *       <t t-name="sale.SpecialForm" t-inherit="sale.OrderForm">   nouveau template dérivé
     *           <xpath expr="//h1" position="replace"><h2>...</h2></xpath>
     *       </t>
     *   </templates>
     *
     * Les fichiers sont ajoutés dans l'ordre des dépendances des modules : les extensions s'appliquent
     * dans cet ordre. Les nœuds gardent leur fichier et leur ligne d'origine (messages d'erreur).
     */
    import { type XElement, type XNode } from "trame/internal/compiler/xml";
    export class TemplateLibrary {
        private readonly definitions;
        private readonly extensions;
        /** Ajoute un fichier de templates. Renvoie les noms définis et les extensions du fichier. */
        addFile(content: string, path: string): {
            defined: string[];
            extensions: {
                target: string;
                ops: XElement[];
            }[];
        };
        /** Extensions dont la cible n'est définie dans aucun fichier. */
        unknownTargets(): {
            target: string;
            origin: string;
        }[];
        has(name: string): boolean;
        names(): string[];
        /** Arbre final d'un template (base, héritage primaire, extensions), recalculé à chaque appel. */
        resolve(name: string, seen?: string[]): XNode[];
    }
}

declare module "trame/internal/compiler/index" {
    /**
     * Compilateur de templates autonome (sans DOM) : utilisable hors du navigateur, par exemple par le
     * serveur de l'ERP qui précompile les templates au démarrage (QuickJS embarqué, Node...).
     *
     *   // Tous les fichiers de templates des modules installés, dans l'ordre des dépendances :
     *   const js = compileTemplateFiles([{ path: "sale/static/order.xml", content: "..." }, ...]);
     *   // js : module JS à servir au navigateur, qui enregistre les templates compilés.
     *
     *   // Ou un seul template :
     *   const factory = compileTemplate(xmlSource, { name: "sale.OrderForm", extensions: [ext1] });
     */
    import { type CompileMode } from "trame/internal/compiler/codegen";
    import { TemplateLibrary } from "trame/internal/compiler/files";
    export interface CompileOptions {
        /** Nom du template (messages d'erreur). */
        name?: string;
        /** Extensions xpath à appliquer, dans l'ordre. */
        extensions?: string[];
        /** "component" (défaut) pour un template de composant, "call" pour un template appelé par t-call. */
        mode?: CompileMode;
    }
    /** Compile un template XML en code JS : une fabrique `(function ($h) { ... })`. */
    export function compileTemplate(source: string, options?: CompileOptions): string;
    export interface TemplateFile {
        /** Chemin (ou nom) du fichier : repris dans les messages d'erreur. */
        path: string;
        content: string;
    }
    export interface CompileFilesOptions {
        /** Module d'où importer Trame dans le code généré (défaut : "trame"). */
        module?: string;
    }
    /**
     * Compile tous les templates de fichiers XML (dans l'ordre des dépendances des modules) et renvoie
     * un module JS qui les enregistre :
     *
     *   import { registerCompiled } from "trame";
     *   registerCompiled("sale.OrderForm", { component: (function ($h) { ... }) });
     *
     * Les templates appelés par t-call sont aussi compilés dans ce mode.
     * Une erreur (syntaxe, extension invalide...) lève une exception qui indique le fichier et la ligne.
     */
    export function compileTemplateFiles(files: TemplateFile[], options?: CompileFilesOptions): string;
    export { TemplateLibrary };
    export type { CompileMode };
}

declare module "trame/internal/compiler/parser" {
    /**
     * Transforme l'arbre XML d'un template en AST, en interprétant les directives t-*.
     *
     * Directives :
     *   t-if / t-elif / t-else          conditions (sur des éléments frères consécutifs)
     *   t-foreach + t-as [+ t-key]      boucle (t-key fortement recommandé)
     *   t-set + t-value                 variable locale (réactive)
     *   t-out                           insère une valeur (texte échappé, ou markup(...) pour du HTML)
     *   t-att-NAME / t-att              attributs dynamiques
     *   NAME="... {{ expr }} ..."       interpolation dans un attribut
     *   {{ expr }}                      interpolation dans le texte
     *   t-on-EVENT[.modificateurs]      événements
     *   t-ref                           référence vers l'élément
     *   t-component                     composant dynamique
     *   t-props                         props passées en bloc à un composant
     *   t-slot [+ attributs]            (dans un composant) affiche un slot reçu, avec contenu par défaut
     *   t-set-slot [+ t-slot-scope]     (dans l'appel d'un composant) définit un slot nommé
     *   t-call                          appelle un template nommé
     *   t-key (hors t-foreach)          recrée l'élément / le composant quand la valeur change
     */
    import { TemplateSyntaxError, type XNode, type XPosition } from "trame/internal/compiler/xml";
    export type Namespace = "html" | "svg" | "math";
    /** Position dans la source (ligne, origine) : utilisée pour localiser les erreurs. */
    export type Pos = XPosition;
    export interface TextPart {
        /** Texte statique ou expression. */
        text?: string;
        expr?: string;
    }
    export interface ASTText {
        type: "text";
        value: string;
        /** Ne pas traduire (t-translation="off"). */
        raw?: boolean;
    }
    export interface ASTTextExpr {
        type: "textExpr";
        parts: TextPart[];
        pos?: Pos;
    }
    export interface ASTAttrExpr {
        name: string;
        /** Expression (t-att-x) ou morceaux interpolés (x="a {{ b }}"). */
        expr?: string;
        parts?: TextPart[];
    }
    export interface ASTEvent {
        name: string;
        modifiers: string[];
        expr: string;
    }
    export interface ASTElement {
        type: "element";
        tag: string;
        ns: Namespace;
        attrs: [string, string][];
        dynAttrs: ASTAttrExpr[];
        attrsSpread?: string;
        events: ASTEvent[];
        ref?: string;
        children: AST[];
        /** Ne pas traduire les attributs (t-translation="off"). */
        noTranslate?: boolean;
        pos?: Pos;
    }
    export interface ASTMulti {
        type: "multi";
        children: AST[];
    }
    export interface ASTIf {
        type: "if";
        branches: {
            cond: string | null;
            body: AST;
        }[];
        pos?: Pos;
    }
    export interface ASTForEach {
        type: "foreach";
        collection: string;
        as: string;
        key: string | null;
        body: AST;
        pos?: Pos;
    }
    export interface ASTSet {
        type: "set";
        name: string;
        value: string;
        pos?: Pos;
    }
    export interface ASTOut {
        type: "out";
        expr: string;
        pos?: Pos;
    }
    export interface ASTSlotDef {
        name: string;
        scope: string | null;
        body: AST;
    }
    export interface ASTComponent {
        type: "component";
        name: string | null;
        dynamic: string | null;
        props: {
            name: string;
            expr?: string;
            parts?: TextPart[];
        }[];
        spread: string | null;
        slots: ASTSlotDef[];
        pos?: Pos;
    }
    export interface ASTSlot {
        type: "slot";
        name: string;
        params: {
            name: string;
            expr: string;
        }[];
        fallback: AST | null;
        pos?: Pos;
    }
    /** t-key hors d'une boucle : le contenu est recréé quand la clé change. */
    export interface ASTKeyed {
        type: "keyed";
        key: string;
        body: AST;
        pos?: Pos;
    }
    export interface ASTCall {
        type: "call";
        template: string;
        params: {
            name: string;
            expr: string;
        }[];
        pos?: Pos;
    }
    export type AST = ASTText | ASTTextExpr | ASTElement | ASTMulti | ASTIf | ASTForEach | ASTSet | ASTOut | ASTComponent | ASTSlot | ASTCall | ASTKeyed;
    /** Composants fournis par Trame, utilisables sans les déclarer dans `static components`. */
    export const BUILTIN_COMPONENTS: Set<string>;
    export function parseTemplate(nodes: XNode[]): AST;
    /** Découpe « texte {{ expr }} texte » en morceaux. */
    export function splitInterpolation(text: string): TextPart[] | null;
    export { TemplateSyntaxError };
}

declare module "trame/internal/compiler/typecheck" {
    /**
     * Vérification des templates par TypeScript.
     *
     * Pour chaque template, on génère du code TypeScript « fantôme » qui reprend chaque expression avec
     * les vrais types : membres du composant, variables de boucle, paramètres d'événements (typés selon
     * la balise), props des composants enfants (vérifiées contre leur schéma props()). Ce code n'est
     * jamais exécuté : il est seulement soumis à `tsc`, et chaque ligne générée est reliée à la ligne
     * du template, pour que les erreurs soient signalées au bon endroit.
     */
    import { type Pos } from "trame/internal/compiler/parser";
    import { type XNode } from "trame/internal/compiler/xml";
    export interface CheckResult {
        /** Lignes du code généré (corps d'une méthode statique à insérer dans la classe). */
        lines: string[];
        /** Pour chaque ligne générée : la ligne du template (relative, à partir de 1), si le nœud vient du template lui-même. */
        templateLines: (number | undefined)[];
        /** Pour chaque ligne générée : position complète (ligne et fichier d'origine pour un fichier de templates). */
        positions: (Pos | undefined)[];
    }
    /** Déclarations utilisées par le code généré (à ajouter une fois par fichier, au niveau du module). */
    export function checkPrelude(trameModule: string): string;
    /**
     * Génère le code de vérification d'un template.
     * @param className  classe du composant (référencée comme type de `$c`)
     */
    export function generateCheck(source: string, className: string): CheckResult;
    /** Même chose à partir d'un arbre déjà résolu (fichier de templates avec ses extensions). */
    export function generateCheckFromNodes(nodes: XNode[], className: string): CheckResult;
}

declare module "trame/internal/compiler/xml" {
    /**
     * Petit parseur XML, sans dépendance au DOM (utilisable aussi côté serveur pour précompiler).
     * Il produit un arbre simple, manipulable par l'héritage de templates (xpath).
     */
    export interface XAttr {
        name: string;
        value: string;
    }
    /** Position d'un nœud dans sa source (messages d'erreur). */
    export interface XPosition {
        /** Ligne (à partir de 1) dans la source du template ou de l'extension. */
        line?: number;
        /** Source d'origine quand ce n'est pas le template lui-même (ex. une extension). */
        origin?: string;
    }
    export interface XElement extends XPosition {
        type: "element";
        tag: string;
        attrs: XAttr[];
        children: XNode[];
        parent: XElement | null;
    }
    export interface XText extends XPosition {
        type: "text";
        value: string;
        parent: XElement | null;
    }
    export type XNode = XElement | XText;
    export class TemplateSyntaxError extends Error {
        constructor(message: string, source: string, index: number);
    }
    export function decodeEntities(text: string): string;
    /** Parse une chaîne XML (plusieurs racines autorisées) en liste de nœuds. */
    export function parseXML(source: string, origin?: string): XNode[];
    export function getAttr(el: XElement, name: string): string | undefined;
    export function hasAttr(el: XElement, name: string): boolean;
    export function setAttr(el: XElement, name: string, value: string): void;
    export function removeAttr(el: XElement, name: string): void;
    export function cloneNode<T extends XNode>(node: T, parent?: XElement | null): T;
    /** Resérialise un arbre (utile pour les messages d'erreur et le débogage). */
    export function serialize(nodes: XNode[]): string;
}

declare module "trame/internal/compiler/xpath" {
    /**
     * Héritage de templates (façon Odoo) : une extension modifie l'arbre XML d'un template avant compilation.
     *
     *   <t>
     *     <xpath expr="//h1" position="after"><p>Ajouté</p></xpath>
     *     <xpath expr="//table" position="attributes">
     *       <attribute name="class" add="big" remove="small"/>
     *       <attribute name="title">Lignes</attribute>
     *     </xpath>
     *     <div name="footer" position="inside">...</div>   (raccourci : premier <div name="footer">)
     *   </t>
     *
     * Positions : inside (à la fin), before, after, replace (le texte « $0 » réinsère l'élément d'origine),
     * attributes.
     *
     * Sous-ensemble xpath supporté : /a/b, //a, *, ., .., prédicats [n], [@attr], [@attr='v'],
     * [hasclass('x')], [contains(@attr, 'v')], combinés avec « and ».
     */
    import { type XElement, type XNode } from "trame/internal/compiler/xml";
    export function applyExtension(nodes: XNode[], extension: string, templateName: string, origin?: string): void;
    /** Opérations d'une extension : les <xpath>/éléments positionnés, éventuellement regroupés dans un <t>. */
    export function extensionOperations(nodes: XNode[]): XElement[];
    /** Applique des opérations d'extension (déjà parsées, avec leurs positions d'origine). */
    export function applyOperations(nodes: XNode[], ops: XElement[], templateName: string): void;
}

declare module "trame/internal/decorators" {
    /**
     * Décorateurs : l'unique façon de déclarer de la réactivité dans Trame.
     *
     *   @state accessor qty = 1;                        état réactif (objets/tableaux : réactifs en profondeur)
     *   @computed get total() { ... }                   valeur dérivée, paresseuse et mise en cache
     *   @resource accessor order = load(({ signal }) => fetchOrder(this.props.id, signal));
     *                                                   donnée asynchrone, chargée à la première lecture
     *   @effect draw() { ...; return () => cleanup }    effet de bord (après le montage pour un composant)
     *   @provide editor = new OrderEditor(this);        fournit un service au sous-arbre
     *   @inject(Rpc) rpc!: Rpc;                         récupère un service fourni par un ancêtre ou l'app
     *
     * Ils fonctionnent dans les composants, les plugins et n'importe quelle classe.
     * Ce sont des décorateurs standard (TypeScript 5+, sans experimentalDecorators).
     */
    import { Owner } from "trame/internal/reactivity/owner";
    import { type Fetcher, type ResourceOptions, type SourcedFetcher } from "trame/internal/reactivity/resource";
    /**
     * État réactif : `@state accessor qty = 1;`
     *
     * La valeur est gardée dans le stockage privé de l'accessor. Le signal n'est créé qu'à la première
     * lecture suivie (liaison de template, @computed, @effect...) : un champ jamais affiché ni observé ne
     * coûte pas plus qu'un champ ordinaire. Avant cela, une écriture n'a personne à prévenir.
     */
    export function state<This extends object, V>(target: ClassAccessorDecoratorTarget<This, V>, context: ClassAccessorDecoratorContext<This, V>): ClassAccessorDecoratorResult<This, V>;
    export interface ComputedDecoratorOptions {
        /**
         * Préchargement : la valeur est calculée dès la construction de l'objet, même si rien ne la lit
         * (contenu d'un t-if fermé...), et recalculée quand ses dépendances changent. Les données qu'elle
         * lit sont donc chargées d'avance. L'affichage n'attend pas ce préchargement, et une erreur n'y
         * est pas signalée (elle le sera là où la valeur est lue).
         */
        eager?: boolean;
    }
    type GetterDecorator = <This extends object, V>(getter: (this: This) => V, context: ClassGetterDecoratorContext<This, V>) => (this: This) => V;
    /**
     * Valeur dérivée, paresseuse et mise en cache :
     *   @computed get total() { ... }
     *   @computed({ eager: true }) get partnerName() { ... }    préchargée (voir ComputedDecoratorOptions)
     */
    export function computed<This extends object, V>(getter: (this: This) => V, context: ClassGetterDecoratorContext<This, V>): (this: This) => V;
    export function computed(options: ComputedDecoratorOptions): GetterDecorator;
    /**
     * Déclare le chargeur d'une @resource. Le type renvoyé est celui de la donnée.
     *
     * Avec une source explicite, seules les valeurs lues par la source sont suivies, et le fetcher reçoit
     * sa valeur (rien de ce qu'il lit n'est suivi, même après un `await`) :
     *   @resource accessor order = load(() => this.props.orderId, (id, { signal }) => fetchOrder(id, signal));
     *
     * Sans source, ce que le fetcher lit avant son premier `await` est suivi :
     *   @resource accessor order = load(({ signal }) => fetchOrder(this.props.orderId, signal));
     *
     * Si une dépendance change, la donnée est rechargée.
     */
    export function load<T>(fetcher: Fetcher<T>, options?: ResourceOptions): T;
    export function load<S, T>(source: () => S, fetcher: SourcedFetcher<S, T>, options?: ResourceOptions): T;
    /** Donnée asynchrone, chargée à la première lecture. */
    export function resource<This extends object, V>(_target: ClassAccessorDecoratorTarget<This, V>, context: ClassAccessorDecoratorContext<This, V>): ClassAccessorDecoratorResult<This, V>;
    /**
     * Effet de bord : la méthode s'exécute (après le montage pour un composant), puis à chaque
     * changement des valeurs qu'elle lit. Elle peut renvoyer une fonction de nettoyage.
     *
     * Chaque exécution a son propre scope : les objets qu'elle crée (avec des @resource ou des @effect)
     * sont nettoyés avant l'exécution suivante et à la destruction du propriétaire.
     */
    export function effect<This extends object>(_method: (this: This) => void | (() => void), context: ClassMethodDecoratorContext<This, (this: This) => void | (() => void)>): void;
    type Key<T = unknown> = abstract new (...args: never[]) => T;
    /**
     * Enregistre un service sur un scope (utilisé par @provide et par mount({ provide })).
     * Il est fourni sous sa classe exacte (ou sous `key`), et aussi sous ses classes parentes : on peut
     * injecter une classe parente, tant qu'un seul service du scope en hérite.
     */
    export function provideOn(owner: Owner, value: unknown, key?: Key): void;
    /** Récupère un service fourni par le scope courant, un ancêtre ou l'application. */
    export function lookupService<T>(key: Key<T>): T;
    /**
     * Fournit la valeur du champ à tout le sous-arbre :
     *   @provide editor = new OrderEditor(this);      (clé : la classe de la valeur et ses parentes)
     *   @provide(Editor) editor = new OrderEditor(this);
     */
    export function provide<This, V>(target: undefined, context: ClassFieldDecoratorContext<This, V>): (value: V) => V;
    export function provide<T>(key: Key<T>): <This, V extends T>(target: undefined, context: ClassFieldDecoratorContext<This, V>) => (value: V) => V;
    /** Récupère un service : `@inject(Rpc) rpc!: Rpc;` */
    export function inject<T>(key: Key<T>): <This>(_target: undefined, _context: ClassFieldDecoratorContext<This, T>) => (value: T) => T;
}

declare module "trame/internal/i18n" {
    /**
     * Traductions.
     *
     * Les textes statiques des templates, ainsi que les attributs title, placeholder, alt, aria-label et
     * label, passent par la fonction de traduction. Elle est appliquée une seule fois, à la construction
     * de la partie statique d'un template : définissez-la avant le premier montage.
     *
     *   setTranslator((text) => translations[text] ?? text);
     *   _t("Enregistrer")                      // dans le code (ou dans une expression de template)
     *   <div t-translation="off">SO001</div>  // ne pas traduire un sous-arbre
     */
    /** Définit la fonction de traduction (null pour désactiver). */
    export function setTranslator(fn: ((text: string) => string) | null): void;
    /** Traduit un texte (renvoyé tel quel sans traducteur). */
    export function _t(text: string): string;
    /** Traduit un texte de template en conservant les blancs autour. */
    export function translateTemplateText(text: string): string;
    export const TRANSLATABLE_ATTRIBUTES: Set<string>;
}

declare module "trame/internal/index" {
    /**
     * Trame : framework TypeScript réactif à base de signaux.
     *
     * Version complète : inclut le compilateur de templates (compilés dans le navigateur au premier
     * affichage). Voir index.runtime.ts pour la version sans compilateur.
     */
    import "trame/internal/runtime/compiler_setup";
    export * from "trame/internal/api";
}

declare module "trame/internal/index.runtime" {
    /**
     * Trame, version sans compilateur de templates (trame.runtime.js).
     *
     * Les templates doivent être précompilés, par exemple par le serveur avec compileTemplateFiles
     * (trame-compiler.js), puis enregistrés avec registerCompiled.
     */
    export * from "trame/internal/api";
}

declare module "trame/internal/patch" {
    /**
     * patch() : extension d'une classe (ou d'un objet) existante, en place, façon Odoo.
     *
     *   patch(OrderLine, class extends OrderLine {
     *       @state accessor ecoTax = 0;                       // nouveau champ réactif
     *       @computed get total() { return super.total + this.ecoTax; }   // surcharge avec super
     *   });
     *
     *   patch(OrderForm, { save() { console.log("avant"); return super.save(); } });
     *
     * - Les méthodes, getters et setters sont installés sur le prototype de la classe cible :
     *   toutes les instances (existantes et futures), et les sous-classes, en profitent.
     * - `super` appelle l'implémentation précédente (patchs empilables).
     * - Les champs déclarés dans une classe de patch (y compris @state, @resource, @provide...) sont
     *   initialisés sur chaque instance : dès la construction pour les composants, et au premier accès
     *   à un membre du patch pour les autres objets.
     * - patch() renvoie une fonction qui annule le patch (à utiliser dans l'ordre inverse d'application).
     */
    type AnyClass = abstract new (...args: never[]) => object;
    /** Initialise sur `instance` les champs de tous les patchs qui la concernent (idempotent). */
    export function initPatches(instance: object): void;
    /**
     * Étend `target` (classe ou objet) avec `extension` (classe `extends target` ou objet littéral).
     * Renvoie une fonction d'annulation.
     */
    export function patch<T extends AnyClass | object>(target: T, extension: object): () => void;
}

declare module "trame/internal/props" {
    /**
     * Déclaration des props d'un composant :
     *
     *   props = props({
     *       orderId: t.number(),
     *       readonly: t.boolean().default(false),
     *       onSaved: t.func<(order: Order) => void>().optional(),
     *   });
     *
     * Une seule déclaration donne : le type TS de `this.props`, la validation en mode dev
     * (types, props manquantes ou inconnues) et les valeurs par défaut.
     *
     * Les props sont en lecture seule, en profondeur : l'enfant ne modifie jamais ce que le parent lui
     * passe, il le prévient via un callback. C'est garanti par le type (DeepReadonly) et par trame-check.
     * À l'exécution, l'enfant reçoit les objets mêmes du parent, en dev comme en prod : seul l'objet
     * `this.props` refuse les écritures.
     */
    export class Validator<T, Optional extends boolean = false, HasDefault extends boolean = false> {
        readonly describe: string;
        private readonly test;
        readonly isOptional: boolean;
        readonly hasDefault: boolean;
        readonly defaultValue: unknown;
        readonly nullable: boolean;
        readonly __type: T;
        readonly __optional: Optional;
        readonly __default: HasDefault;
        constructor(describe: string, test: (value: unknown) => string | null, isOptional?: boolean, hasDefault?: boolean, defaultValue?: unknown, nullable?: boolean);
        /** Message d'erreur, ou null si la valeur est valide. */
        check(value: unknown): string | null;
        /** Prop facultative. */
        optional(): Validator<T, true, HasDefault>;
        /** Valeur par défaut si la prop n'est pas passée (ou vaut undefined). */
        default(value: T): Validator<T, false, true>;
        /** Accepte aussi null. */
        orNull(): Validator<T | null, Optional, HasDefault>;
    }
    type AnyValidator = Validator<unknown, boolean, boolean>;
    type Shape = Record<string, AnyValidator>;
    /** Validateurs de types pour props(). */
    export const t: {
        string: () => Validator<string, false, false>;
        number: () => Validator<number, false, false>;
        boolean: () => Validator<boolean, false, false>;
        func: <F extends (...args: never[]) => unknown = (...args: unknown[]) => unknown>() => Validator<F>;
        any: <T = unknown>() => Validator<T>;
        instanceOf: <C extends abstract new (...args: never[]) => unknown>(ctor: C) => Validator<InstanceType<C>>;
        array: <V extends AnyValidator | undefined = undefined>(item?: V) => Validator<V extends AnyValidator ? V["__type"][] : unknown[]>;
        object: <S extends Shape | undefined = undefined>(shape?: S) => Validator<S extends Shape ? InferShape<S> : Record<string, unknown>>;
        literal: <const L extends readonly (string | number | boolean | null)[]>(...values: L) => Validator<L[number], false, false>;
        or: <V extends AnyValidator[]>(...validators: V) => Validator<V[number]["__type"], false, false>;
    };
    type RequiredKeys<S extends Shape> = {
        [K in keyof S]: S[K]["__optional"] extends true ? never : K;
    }[keyof S];
    type OptionalKeys<S extends Shape> = {
        [K in keyof S]: S[K]["__optional"] extends true ? K : never;
    }[keyof S];
    type Simplify<T> = {
        [K in keyof T]: T[K];
    } & {};
    export type InferShape<S extends Shape> = Simplify<{
        [K in RequiredKeys<S>]: S[K]["__type"];
    } & {
        [K in OptionalKeys<S>]?: S[K]["__type"];
    }>;
    /** Lecture seule profonde (les fonctions restent appelables). */
    export type DeepReadonly<T> = T extends (...args: never[]) => unknown ? T : T extends object ? {
        readonly [K in keyof T]: DeepReadonly<T[K]>;
    } : T;
    /** Marque (type seulement) : conserve le schéma dans le type de `this.props`. */
    export const PROPS_SCHEMA: unique symbol;
    export type PropsOf<S extends Shape> = DeepReadonly<InferShape<S>> & {
        readonly [PROPS_SCHEMA]?: S;
    };
    /**
     * Props à passer au composant, vues du parent : celles qui ont une valeur par défaut ou qui sont
     * facultatives peuvent être omises.
     */
    export type PropsInputOf<S extends Shape> = Simplify<{
        [K in keyof S as S[K]["__optional"] extends true ? never : S[K]["__default"] extends true ? never : K]: S[K]["__type"];
    } & {
        [K in keyof S as S[K]["__optional"] extends true ? K : S[K]["__default"] extends true ? K : never]?: S[K]["__type"];
    }>;
    /** Props attendues par une classe de composant (utilisé par la vérification des templates). */
    export type ComponentPropsInput<C> = C extends abstract new (...args: never[]) => {
        props: infer P;
    } ? P extends {
        readonly [PROPS_SCHEMA]?: infer S;
    } ? S extends Shape ? PropsInputOf<S> : Record<string, unknown> : Record<string, unknown> : Record<string, never>;
    /**
     * Déclare (et valide en mode dev) les props du composant en cours de construction.
     * À utiliser dans un champ : `props = props({ ... })`. Le schéma est lu une fois par classe (celui de
     * la première instance) : il ne doit pas dépendre de l'instance.
     */
    export function props<S extends Shape>(schema: S): PropsOf<S>;
}

declare module "trame/internal/reactivity/core" {
    /**
     * Moteur réactif de Trame.
     *
     * Graphe push-pull :
     * - une écriture pousse un marquage (DIRTY sur les observateurs directs, CHECK plus loin) ;
     * - une lecture tire les valeurs : un nœud CHECK vérifie ses sources avant de décider s'il se recalcule.
     *
     * Les nœuds « vivants » (effets, et computed observés) sont abonnés à leurs sources.
     * Un computed non observé n'est abonné à rien : il se revalide à la lecture grâce aux numéros de version,
     * ce qui le rend collectable par le ramasse-miettes dès qu'on ne le référence plus.
     */
    import type { Owner } from "trame/internal/reactivity/owner";
    export type Equals<T> = (a: T, b: T) => boolean;
    /** Une ressource asynchrone « en attente » (premier chargement), vue par le moteur. */
    export interface PendingSource {
        /** Boundaries en attente de cette ressource. */
        waiters: Set<PendingWaiter>;
    }
    export interface PendingWaiter {
        resolved(source: PendingSource): void;
        failed(source: PendingSource, error: unknown): void;
    }
    export function getCurrentObserver(): Computation | null;
    /** Exécute `fn` sans enregistrer de dépendances. */
    export function untrack<T>(fn: () => T): T;
    /**
     * Une dépendance entre une source et un calcul : une seule allocation par dépendance, réutilisée
     * d'une exécution à l'autre tant que la source est relue.
     * - Liste des sources du calcul (simplement chaînée, dans l'ordre de lecture) : `nextSource`.
     * - Liste des observateurs de la source (doublement chaînée, seulement si le calcul est vivant) :
     *   `prevObserver` / `nextObserver`. Le désabonnement se fait en O(1).
     */
    class Link {
        readonly source: ReactiveNode;
        readonly observer: Computation;
        /** Version de la source vue à la dernière lecture ; -1 : pas encore relue pendant l'exécution en cours. */
        version: number;
        nextSource: Link | null;
        /** Liste des sources en construction pendant l'exécution. */
        nextTracked: Link | null;
        prevObserver: Link | null;
        nextObserver: Link | null;
        /** Lien courant précédent de la source (exécutions imbriquées), restauré en fin d'exécution. */
        rollback: Link | undefined;
        /** Créé pendant l'exécution en cours : à abonner à la fin si le calcul est vivant. */
        fresh: boolean;
        constructor(source: ReactiveNode, observer: Computation);
    }
    export abstract class ReactiveNode {
        /** Incrémentée à chaque changement de valeur. */
        version: number;
        /** Observateurs vivants (liste doublement chaînée de liens). */
        private observersHead;
        private observersTail;
        /**
         * Pendant l'exécution d'un calcul qui lit ce nœud : son lien vers ce calcul. Permet de réutiliser
         * le lien de l'exécution précédente et d'ignorer les lectures en double, sans structure auxiliaire.
         */
        currentLink: Link | undefined;
        protected track(): void;
        /** Abonne un lien (calcul vivant) à ce nœud. */
        linkObserver(link: Link): void;
        /** Désabonne un lien ; prévient le nœud s'il n'a plus d'observateur. */
        unlinkObserver(link: Link): void;
        get observed(): boolean;
        /** Nombre d'observateurs vivants (diagnostic, tests). */
        get observerCount(): number;
        /** Marque tous les observateurs vivants. */
        protected markObservers(state: number): void;
        /** Appelé quand le dernier observateur vivant disparaît. */
        protected onUnobserved(): void;
        /** Appelé quand une lecture veut s'assurer que la valeur est à jour. */
        updateIfNeeded(): void;
        /** Prévient les observateurs que la valeur a changé. */
        protected notify(): void;
    }
    export class Signal<T> extends ReactiveNode {
        private value;
        private readonly equals;
        constructor(value: T, equals?: Equals<T> | false);
        get(): T;
        /** Lit la valeur sans créer de dépendance. */
        peek(): T;
        set(value: T): void;
        /** Force la notification des observateurs (valeur mutée en place). */
        trigger(): void;
    }
    export abstract class Computation extends ReactiveNode {
        state: number;
        /** Sources lues lors de la dernière exécution (liste de liens, dans l'ordre de lecture). */
        private sourcesHead;
        /** Ressources en attente lues (directement ou via un computed) lors de la dernière exécution. */
        pending: Set<PendingSource> | null;
        protected flags: number;
        /** Exécuté au moins une fois (ses sources sont connues) ? */
        protected get tracked(): boolean;
        /** Abonné à ses sources ? */
        get live(): boolean;
        set live(value: boolean);
        get disposed(): boolean;
        set disposed(value: boolean);
        addSource(source: ReactiveNode): void;
        addPending(source: PendingSource): void;
        /** Pendant l'exécution : une ressource en attente a-t-elle été lue ? */
        hasPendingReads(): boolean;
        /** Parcourt les sources lues lors de la dernière exécution. */
        forEachSource(fn: (source: ReactiveNode) => void): void;
        /** Hors exécution : faut-il réexécuter (une source a-t-elle vraiment changé) ? */
        needsUpdate(): boolean;
        /** Exécute `fn` en collectant les dépendances, puis met à jour les abonnements. */
        runTracked<R>(fn: () => R): R;
        /** Fin d'exécution : abandonne les sources non relues, abonne les nouvelles. */
        private commitSources;
        /** Abonne toutes les sources (passage à l'état vivant). */
        protected subscribeAll(): void;
        /** Vérifie si une source a changé depuis la dernière exécution. */
        protected sourcesChanged(): boolean;
        abstract mark(state: number): void;
        protected unsubscribeAll(): void;
    }
    export interface ComputedOptions<T> {
        equals?: Equals<T>;
    }
    export type RecomputeListener = (computed: Computed<unknown>) => void;
    /** Appelé à chaque recalcul d'un computed (utilisé par le mode observation de loading()). Renvoie l'ancien. */
    export function setRecomputeListener(listener: RecomputeListener | null): RecomputeListener | null;
    /** Valeur dérivée, paresseuse et mise en cache. */
    export class Computed<T> extends Computation {
        private readonly fn;
        private value;
        private error;
        private hasError;
        private readonly equals;
        private lastGlobalVersion;
        /** Recalcul forcé à la prochaine lecture (cache calculé en mode observation). */
        private forced;
        constructor(fn: () => T, options?: ComputedOptions<T>);
        get(): T;
        peek(): T;
        /** Force un recalcul à la prochaine lecture. */
        invalidate(): void;
        updateIfNeeded(): void;
        private recompute;
        mark(state: number): void;
        goLive(): void;
        protected onUnobserved(): void;
        dispose(): void;
    }
    /** Priorités d'exécution : les ressources relancées d'abord, puis le DOM, puis les effets utilisateur. */
    export const PRIORITY_RESOURCE = 0;
    export const PRIORITY_RENDER = 1;
    export const PRIORITY_USER = 2;
    export class Effect extends Computation {
        private readonly fn;
        readonly owner: Owner | null;
        /** Clé de tri dans la file : priorité, puis profondeur (parents d'abord), puis ordre de création. */
        readonly sortKey: number;
        private cleanup;
        constructor(fn: () => void | (() => void), owner: Owner | null, priority?: number);
        /** Les ressources en attente lues font-elles attendre l'affichage (frontière du scope) ? */
        get waitsForPending(): boolean;
        get queued(): boolean;
        set queued(value: boolean);
        /** Localisation dans un template (conservée en mode dev seulement). */
        get loc(): string | undefined;
        set loc(value: string | undefined);
        /** Exécute l'effet immédiatement. */
        run(): void;
        protected handleError(e: unknown): void;
        private runCleanup;
        mark(state: number): void;
        schedule(): void;
        /** Marqué (une source a peut-être changé) et pas encore réexécuté ? */
        get needsRun(): boolean;
        /** Appelé par le flush : vérifie les sources puis exécute si nécessaire (pas sous un scope gelé). */
        update(): void;
        dispose(): void;
    }
    const scheduleMicrotask: (fn: () => void) => void;
    /** Exécute tous les effets en attente, puis les callbacks « après flush ». */
    export function flush(): void;
    /** Regroupe plusieurs écritures : les effets s'exécutent une seule fois, à la fin, de façon synchrone. */
    export function batch<T>(fn: () => T): T;
    /**
     * Regroupe des notifications sans forcer d'exécution synchrone : les effets restent planifiés
     * au prochain microtask (sauf si l'on est déjà dans un batch, qui les exécutera à sa fin).
     * Utilisé par le store : une suite d'écritures (ex. échange de deux éléments) n'expose jamais
     * d'état intermédiaire.
     */
    export function groupWrites<T>(fn: () => T): T;
    /** Planifie `fn` après le prochain flush (ou au prochain microtask s'il n'y a rien à exécuter). */
    export function afterFlush(fn: () => void): void;
    /** Promesse résolue une fois que toutes les mises à jour en attente ont été appliquées. */
    export function nextTick(): Promise<void>;
    export function reportError(e: unknown): void;
    /**
     * Mode dev : ajoute à une erreur sa localisation dans le template
     * (« template "OrderForm", ligne 12 : {{ order.total.toFixed(2) }} »).
     *
     * - `error.trameLocation` contient la localisation ;
     * - la pile d'appels (`error.stack`, affichée par la console) la mentionne juste après le message.
     *
     * Le message et le type de l'erreur ne sont pas modifiés (un <ErrorBoundary> affiche un message propre).
     * La première localisation, la plus précise, est conservée.
     */
    export function annotateError(error: unknown, loc: string | undefined, owner: Owner | null): unknown;
    export { scheduleMicrotask };
}

declare module "trame/internal/reactivity/owner" {
    /**
     * Arbre de propriété (scopes).
     *
     * Chaque composant, chaque branche de t-if, chaque ligne de t-foreach possède un Owner.
     * Il enregistre les effets et nettoyages créés en son sein, et libère tout quand il est détruit :
     * effets, sous-scopes, callbacks de nettoyage, AbortSignal.
     *
     * Il porte aussi le contexte hérité : services fournis (@provide), gestionnaire d'erreurs,
     * frontière d'attente (Boundary) et application.
     */
    import { Effect, type PendingSource, type PendingWaiter } from "trame/internal/reactivity/core";
    export interface AppContext {
        readonly dev: boolean;
        /** Erreur de rendu (liaison, effet, construction, chargement) interceptée par personne. */
        handleUncaughtError(error: unknown): void;
        /** Erreur d'action (gestionnaire d'événement) interceptée par aucun <ErrorHandler>. */
        handleActionError(error: unknown): void;
    }
    export function getOwner(): Owner | null;
    export function runWithOwner<T>(owner: Owner | null, fn: () => T): T;
    /** Enregistre un nettoyage sur le scope courant. */
    export function onCleanup(fn: () => void): void;
    /** Plusieurs services différents fournis au même niveau sous une même classe parente. */
    export class AmbiguousService {
        candidates: unknown[];
        constructor(candidates: unknown[]);
    }
    export class Owner {
        readonly parent: Owner | null;
        readonly depth: number;
        app: AppContext | null;
        boundary: Boundary | null;
        /** Le scope est-il affiché dans un DOM vivant ? */
        live: boolean;
        /** Contenu préparé mais pas encore inséré (en attente de données). */
        detached: boolean;
        suspended: boolean;
        disposed: boolean;
        private children;
        private effects;
        private cleanups;
        private extra;
        private get x();
        /** Gestionnaire d'erreurs local : renvoie true si l'erreur est prise en charge. */
        get errorHandler(): ((error: unknown) => boolean) | null;
        set errorHandler(handler: ((error: unknown) => boolean) | null);
        /** Gestionnaire des erreurs d'actions (<ErrorHandler>) : renvoie true si l'erreur est prise en charge. */
        get actionHandler(): ((error: unknown) => boolean) | null;
        set actionHandler(handler: ((error: unknown) => boolean) | null);
        constructor(parent?: Owner | null);
        /** AbortSignal déclenché à la destruction du scope. */
        get abortSignal(): AbortSignal;
        registerEffect(effect: Effect): void;
        onCleanup(fn: () => void): void;
        /** Exécute `fn` quand le scope sera affiché (immédiatement s'il l'est déjà). */
        onMount(fn: () => void): void;
        /** Marque le scope (et ses enfants insérés) comme affiché, et déclenche les callbacks onMount. */
        activate(): void;
        /**
         * Gèle le sous-arbre : ses effets (rendu, ressources, @effect) ne s'exécutent plus, ses computed ne
         * sont donc plus relus. Un contenu en cours de remplacement ne réagit plus à un état qui ne le
         * concerne plus (ex. l'enregistrement de l'ancienne vue passé à null).
         */
        suspend(): void;
        /** Dégèle le sous-arbre : les effets marqués pendant le gel se rattrapent au prochain flush. */
        resume(): void;
        /**
         * Cherche un service fourni par ce scope ou un ancêtre (le plus proche l'emporte). À un même
         * niveau, un service fourni sous sa classe exacte l'emporte sur un service dont c'est une classe parente.
         */
        lookup(key: unknown): unknown;
        /** Fournit `value` sous `key` (classe exacte ou clé explicite) : une seule fois par scope. */
        provide(key: unknown, value: unknown, describe?: (value: unknown) => string): void;
        /** Fournit `value` sous une de ses classes parentes. Deux services différents : injection ambiguë. */
        provideInherited(key: unknown, value: unknown): void;
        /** Remplace une valeur fournie (service instancié à la première demande) sous toutes ses clés. */
        replaceProvided(from: unknown, to: unknown): void;
        /** Signale à la frontière d'attente les ressources lues pendant leur premier chargement. */
        waitFor(pending: Set<PendingSource>): void;
        /**
         * Erreur d'une action (gestionnaire d'événement, y compris une promesse rejetée) : elle remonte au
         * <ErrorHandler> le plus proche, sans passer par les <ErrorBoundary> (le contenu reste affiché),
         * puis à l'application (onError, sinon la console ; l'application reste montée).
         */
        handleActionError(error: unknown): void;
        handleError(error: unknown): void;
        dispose(): void;
    }
    /**
     * Frontière d'attente : compte les ressources en premier chargement lues pendant la construction
     * d'un sous-arbre, et prévient quand tout est prêt (ou quand l'une d'elles échoue).
     */
    export class Boundary implements PendingWaiter {
        private readonly onReady;
        private readonly onError;
        private readonly pending;
        /** Premier scope ayant lu chaque ressource attendue : il reçoit l'erreur si elle échoue. */
        private readonly readers;
        private building;
        settled: boolean;
        private closed;
        constructor(onReady: () => void, onError: (error: unknown) => void);
        get isPending(): boolean;
        /** Des ressources en premier chargement ont-elles été lues (et sont-elles attendues) ? */
        get waiting(): boolean;
        wait(source: PendingSource, reader?: Owner): void;
        resolved(source: PendingSource): void;
        /**
         * Une ressource attendue a échoué : l'erreur est transmise au scope qui l'a lue (une
         * <ErrorBoundary> englobante peut ainsi l'intercepter), puis l'attente continue pour le reste.
         */
        failed(source: PendingSource, error: unknown): void;
        private scheduleCheck;
        /** La construction synchrone est terminée : on peut devenir prêt dès que plus rien n'est attendu. */
        done(): void;
        private check;
        /** Abandonne l'attente (contenu détruit avant d'être prêt). */
        cancel(): void;
    }
}

declare module "trame/internal/reactivity/resource" {
    /**
     * Ressources asynchrones (computed asynchrones).
     *
     * - Paresseuses : rien n'est chargé tant que la valeur n'est pas lue.
     * - Une lecture pendant le premier chargement renvoie `undefined` sans interrompre l'exécution,
     *   et signale la ressource « en attente » à la frontière (Boundary) du lecteur : l'affichage attend.
     * - Dépendances : avec une source explicite (`load(source, fetcher)`), seules les valeurs lues par la
     *   source sont suivies, et le fetcher reçoit sa valeur. Sans source, ce que le fetcher lit dans sa
     *   partie synchrone (avant le premier `await`) est suivi. Si une dépendance change, la ressource est
     *   relancée (immédiatement si elle est observée, sinon à la prochaine lecture). La requête
     *   précédente est annulée via AbortSignal.
     * - Pendant un rechargement, l'ancienne valeur reste visible. Les ressources relancées par le même
     *   changement forment une transition : leurs nouvelles valeurs sont appliquées ensemble.
     * - Une requête lancée va jusqu'au bout même si plus personne ne lit la valeur. Elle n'est annulée
     *   que si elle est remplacée (dépendance modifiée, refresh) ou si le scope propriétaire est détruit.
     */
    import { type PendingSource, type PendingWaiter } from "trame/internal/reactivity/core";
    import type { Owner } from "trame/internal/reactivity/owner";
    export interface ResourceContext {
        /** Déclenché si la requête est remplacée ou si le propriétaire est détruit. */
        readonly signal: AbortSignal;
    }
    export type Fetcher<T> = (ctx: ResourceContext) => Promise<T> | T;
    /** Fetcher d'une ressource à source explicite : reçoit la valeur de la source. */
    export type SourcedFetcher<S, T> = (value: S, ctx: ResourceContext) => Promise<T> | T;
    export interface ResourceOptions {
        /** Charge dès la création au lieu d'attendre la première lecture. */
        eager?: boolean;
    }
    export function isAbortError(e: unknown): boolean;
    export class Resource<T> implements PendingSource {
        private readonly fetcher;
        private readonly owner;
        private readonly options;
        private readonly source;
        readonly waiters: Set<PendingWaiter>;
        private readonly valueSig;
        private readonly loadingSig;
        private readonly errorSig;
        private hasValue;
        private stale;
        private runId;
        private controller;
        private unlinkOwner;
        private tracker;
        private transition;
        private disposed;
        constructor(fetcher: Fetcher<T>, owner: Owner | null, options?: ResourceOptions);
        /** Source explicite des dépendances : `fetcher` reçoit sa valeur et n'est pas suivi. */
        constructor(fetcher: SourcedFetcher<never, T>, owner: Owner | null, options: ResourceOptions, source: () => unknown);
        /** Lecture de la valeur : déclenche le chargement si nécessaire. */
        read(): T | undefined;
        /** Écriture locale (mise à jour optimiste) : aucune requête n'est lancée. */
        write(value: T): void;
        isLoading(): boolean;
        getError(): unknown;
        /** Relance la requête. */
        refresh(): void;
        private get observed();
        onDependenciesChanged(): void;
        private start;
        /** Exécuté par le tracker, dans un contexte qui suit les dépendances. */
        execute(): void;
        private settle;
        commitFromTransition(value: unknown): void;
        private commit;
        private fail;
        private notifyWaiters;
        private releaseController;
        private abortCurrent;
        /**
         * Les boundaries qui attendaient cette ressource cessent de l'attendre : détruite (ex. par le
         * fallback d'une <ErrorBoundary>, parfois au milieu de son propre commit), elle ne chargera plus,
         * et un montage en attente resterait bloqué sans erreur.
         */
        dispose(): void;
    }
    /** Vrai si une des ressources lues par `fn` est en cours de chargement. Ne déclenche aucun chargement. */
    export function loading(fn: () => unknown): boolean;
    /** Première erreur d'une des ressources lues par `fn` (ou undefined). */
    export function error(fn: () => unknown): unknown;
    /** Relance la dernière ressource lue par `fn` (par ex. refresh(() => this.order.partner) relance partner). */
    export function refresh(fn: () => unknown): void;
}

declare module "trame/internal/reactivity/store" {
    /**
     * Store profond : un objet ou tableau « simple » est enveloppé dans un Proxy qui crée,
     * à la demande, un signal par clé. Seules les clés réellement lues sont suivies.
     *
     * Les instances de classes ne sont pas enveloppées : elles gèrent leur réactivité
     * elles-mêmes via les décorateurs (@state...). Map et Set sont pris en charge.
     */
    /** Marque un objet pour qu'il ne soit jamais rendu réactif. */
    export function markRaw<T extends object>(obj: T): T;
    /** Renvoie l'objet d'origine derrière un proxy réactif. */
    export function toRaw<T>(value: T): T;
    export function isReactive(value: unknown): boolean;
    /** Rend un objet/tableau/Map/Set réactif en profondeur. Les autres valeurs sont renvoyées telles quelles. */
    export function reactive<T>(value: T): T;
}

declare module "trame/internal/registry" {
    /**
     * Registres : collections ordonnées et réactives, enrichies par les modules.
     *
     *   registry.category("fields").add("char", CharField);
     *   registry.category("fields").add("monetary", MonetaryField, { sequence: 10 });
     *   registry.category("fields").get("char");
     *   registry.category("fields").getAll();     // triés par séquence, puis ordre d'ajout
     *
     * Lire un registre (get, has, getAll...) dans un template ou un @computed abonne à ses changements.
     */
    export interface AddOptions {
        /** Ordre dans getAll() (croissant). Défaut : 50. */
        sequence?: number;
        /** Remplace une entrée existante au lieu de lever une erreur. */
        force?: boolean;
    }
    export class Registry<T = unknown> {
        readonly name: string;
        private readonly entries;
        private readonly categories;
        private readonly version;
        private sorted;
        constructor(name?: string);
        add(key: string, value: T, options?: AddOptions): this;
        get(key: string): T;
        get<D>(key: string, defaultValue: D): T | D;
        has(key: string): boolean;
        remove(key: string): void;
        /** Valeurs triées par séquence. */
        getAll(): T[];
        /** Paires [clé, valeur] triées par séquence. */
        getEntries(): [string, T][];
        get size(): number;
        /** Sous-registre nommé (créé à la demande). */
        category<C = unknown>(name: string): Registry<C>;
        private changed;
    }
    /** Registre global de l'application. */
    export const registry: Registry<unknown>;
}

declare module "trame/internal/runtime/app" {
    /**
     * Montage d'une application.
     *
     *   const root = await mount(OrderForm, document.body, {
     *       props: { orderId: 42 },
     *       provide: [Rpc, new User(...)],     // services disponibles via @inject
     *       dev: true,                          // validations et messages d'erreur détaillés
     *   });
     *   root.destroy();
     *
     * La promesse est résolue une fois le composant inséré, c'est-à-dire quand toutes les données
     * lues pendant sa construction sont chargées.
     */
    import { type Component, type ComponentClass } from "trame/internal/runtime/component";
    export interface MountOptions {
        /** Props du composant racine. */
        props?: object;
        /** Services fournis à toute l'application : classes (instanciées à la demande) ou instances. */
        provide?: unknown[];
        /** Mode développement : validation des props, messages détaillés. */
        dev?: boolean;
        /**
         * Appelé pour une erreur non interceptée :
         * - erreur de rendu hors de toute <ErrorBoundary> (par défaut : console.error, puis destruction) ;
         * - erreur d'action hors de tout <ErrorHandler> (par défaut : console.error ; l'application reste montée).
         */
        onError?: (error: unknown) => void;
    }
    export interface Root<C extends Component = Component> {
        readonly component: C;
        destroy(): void;
    }
    export function mount<C extends Component>(Ctor: ComponentClass<C>, target: Element, options?: MountOptions): Promise<Root<C>>;
}

declare module "trame/internal/runtime/builtins" {
    /**
     * Composants intégrés, utilisables dans tout template sans déclaration :
     *
     *   <Suspense>               son contenu s'affiche à part quand ses données sont prêtes ;
     *     <t t-set-slot="fallback">Chargement…</t>   (affiché en attendant, facultatif)
     *     ...
     *   </Suspense>
     *
     *   <ErrorBoundary>          intercepte les erreurs de son contenu ;
     *     <t t-set-slot="fallback" t-slot-scope="e">Erreur : {{ e.error.message }}
     *        <button t-on-click="e.reset">Réessayer</button></t>
     *     ...
     *   </ErrorBoundary>
     *
     *   <ErrorHandler onError="(e) => notify(e)">   reçoit les erreurs des actions de son contenu
     *     ...                    (gestionnaires d'événements, promesses rejetées) ; le contenu reste affiché
     *   </ErrorHandler>
     *
     *   <Portal target="'#modals'">   insère son contenu ailleurs dans le document.
     *     ...
     *   </Portal>
     */
    import { Component, type Slots } from "trame/internal/runtime/component";
    import { type Root } from "trame/internal/runtime/regions";
    export class Suspense extends Component {
        static customRender: (_: Suspense, slots: Slots | null) => Root[];
    }
    export class ErrorBoundary extends Component {
        static customRender: (_: ErrorBoundary, slots: Slots | null) => Root[];
    }
    /**
     * Reçoit les erreurs des actions de son contenu (gestionnaires d'événements, y compris les promesses
     * rejetées) : `onError` est appelé et le contenu reste affiché. Les erreurs de rendu (liaisons, effets,
     * construction, chargement) ne passent pas par lui : elles vont à <ErrorBoundary>.
     */
    export class ErrorHandler extends Component {
        static customRender: (component: Component, slots: Slots | null) => Root[];
        props: import("trame/internal/index").PropsOf<{
            onError: import("trame/internal/index").Validator<(error: unknown) => void, false, false>;
        }>;
    }
    export class Portal extends Component {
        static customRender: (component: Portal, slots: Slots | null) => Root[];
    }
}

declare module "trame/internal/runtime/compiler_setup" {
    /**
     * Branche le compilateur de templates (version complète de Trame, trame.js).
     * trame.runtime.js n'importe pas ce module : les templates doivent alors être précompilés.
     */
    import { type TemplateCompiler } from "trame/internal/runtime/template";
    export const templateCompiler: TemplateCompiler;
}

declare module "trame/internal/runtime/component" {
    /**
     * Composants.
     *
     * Un composant est une classe : ses champs décorés (@state, @computed, @resource...) forment son état,
     * son template statique décrit son DOM. Il n'est construit qu'une fois : il n'y a pas de re-rendu,
     * chaque liaison du template se met à jour seule.
     */
    import { type Owner } from "trame/internal/reactivity/owner";
    import type { Root } from "trame/internal/runtime/regions";
    import { type Template } from "trame/internal/runtime/template";
    export type Slots = Record<string, (scope?: unknown) => Root[]>;
    export interface ComponentClass<C extends Component = Component> {
        new (): C;
        template?: Template | string;
        components?: Record<string, ComponentClass>;
        /** Rendu personnalisé (composants internes : Suspense, ErrorBoundary, Portal). */
        customRender?: (instance: C, slots: Slots | null) => Root[];
    }
    interface ConstructionContext {
        /** Classe du composant (le schéma des props est mis en cache par classe). */
        Ctor: Function;
        props: object;
        slots: Slots | null;
        owner: Owner;
        /** Nom du composant (messages d'erreur). */
        name: string;
    }
    /** Contexte du composant en cours de construction (utilisé par props()). */
    export function getConstruction(): ConstructionContext | null;
    /** Composants intégrés (Suspense, ErrorBoundary, Portal), utilisables sans déclaration. */
    export const builtinComponents: Record<string, ComponentClass>;
    /**
     * Classe de base des composants.
     *
     *   class OrderForm extends Component {
     *       static template = xml`...`;
     *       static components = { OrderLineRow };
     *       props = props({ orderId: t.number() });
     *       ...
     *   }
     *
     * `template`, `components` et `props` ne sont volontairement pas déclarés ici : les sous-classes
     * les définissent sans avoir besoin du mot-clé `override`.
     */
    export class Component {
        constructor();
    }
    /** Scope d'un composant (pour les utilitaires internes). */
    export function ownerOf(component: Component): Owner;
    export function slotsOf(component: Component): Slots | null;
    /**
     * Construit un composant sous le scope courant (qui devient le sien) et renvoie ses racines DOM.
     */
    export function renderComponent<C extends Component>(Ctor: ComponentClass<C>, props: object, slots: Slots | null): {
        component: C;
        roots: Root[];
    };
}

declare module "trame/internal/runtime/dom" {
    /**
     * Opérations DOM utilisées par le code généré des templates.
     */
    type Spec = string | {
        r: string;
    } | [string, [string, string][] | 0, Spec[] | 0, number, 1?];
    /**
     * Partie statique d'un bloc : construite une seule fois (à la première utilisation), puis clonée.
     * Construire via createElement (et non innerHTML) évite les corrections du parseur HTML
     * (ex. <tr> hors de <tbody>) qui fausseraient la navigation vers les nœuds dynamiques.
     */
    export function tpl(specs: Spec[], fragment: number): () => Node;
    /** Conversion d'une valeur en texte affiché. */
    export function toText(value: unknown): string;
    export function bindText(node: Text, fn: () => string, loc?: string): void;
    export function bindAttr(el: Element, name: string, fn: () => unknown, loc?: string): void;
    /** Attributs dynamiques en bloc (t-att="{...}"). */
    export function bindAttrs(el: Element, fn: () => unknown, loc?: string): void;
    /**
     * Propriétés de formulaire (value, checked...) : on écrit la propriété, pas l'attribut
     * (l'attribut n'est que la valeur initiale une fois que l'utilisateur a tapé).
     */
    export function bindProp(el: Element, name: string, fn: () => unknown, loc?: string): void;
    /** Classes dynamiques : chaîne, tableau ou objet { classe: condition }. Les classes statiques sont conservées. */
    export function bindClass(el: Element, fn: () => unknown, loc?: string): void;
    /** Style dynamique : chaîne CSS ou objet { propriété: valeur }. */
    export function bindStyle(el: HTMLElement | SVGElement, fn: () => unknown, loc?: string): void;
    /**
     * Événement. Modificateurs : prevent, stop, self, capture, once, passive.
     * Le gestionnaire s'exécute en batch (le DOM est à jour dès qu'il se termine) et ses erreurs,
     * y compris celles d'une promesse renvoyée, remontent au gestionnaire d'erreurs du composant.
     */
    export function bindEvent(el: Element, type: string, handler: (ev: Event) => unknown, modifiers: string, loc?: string): void;
    /** t-ref : affecte l'élément tout de suite, et null à la destruction. */
    export function bindRef(el: Element, setter: (el: Element | null) => void): void;
}

declare module "trame/internal/runtime/helpers" {
    /**
     * Fonctions appelées par le code généré des templates (`$h.xxx`).
     */
    import { _t } from "trame/internal/i18n";
    import { Computed } from "trame/internal/reactivity/core";
    import { error, loading, refresh } from "trame/internal/reactivity/resource";
    import { type Component, type Slots } from "trame/internal/runtime/component";
    import { bindAttr, bindAttrs, bindClass, bindEvent, bindProp, bindRef, bindStyle, bindText, toText, tpl } from "trame/internal/runtime/dom";
    import { type BlockBuilder, ListRegion, markup, OutRegion, type Root, StaticRegion, SwitchRegion } from "trame/internal/runtime/regions";
    /** Props = getters explicites + objet dynamique (t-props). */
    function spreadProps(explicit: Record<string, unknown>, spread: () => unknown): object;
    export const helpers: {
        tpl: typeof tpl;
        s: typeof toText;
        text: typeof bindText;
        attr: typeof bindAttr;
        attrs: typeof bindAttrs;
        cls: typeof bindClass;
        style: typeof bindStyle;
        prop: typeof bindProp;
        on: typeof bindEvent;
        ref: typeof bindRef;
        markup: typeof markup;
        _t: typeof _t;
        loading: typeof loading;
        error: typeof error;
        refresh: typeof refresh;
        computed<T>(fn: () => T): Computed<T>;
        sw(anchor: Node, keyFn: () => number, builders: BlockBuilder[], loc?: string): SwitchRegion;
        /** t-key hors d'une boucle : même contenu, recréé quand la clé change. */
        keyed(anchor: Node, keyFn: () => unknown, builder: BlockBuilder, loc?: string): SwitchRegion;
        each(anchor: Node, listFn: () => unknown, keyFn: ((item: unknown, index: number) => unknown) | null, rowFn: (item: unknown, index: unknown) => Root[], loc?: string, withIndex?: number): ListRegion;
        out(anchor: Node, fn: () => unknown, loc?: string): OutRegion;
        comp(anchor: Node, parent: Component, name: string, props: object, slots: Slots | null, loc?: string, solo?: number): StaticRegion;
        dyn(anchor: Node, parent: Component, fn: () => unknown, props: object, slots: Slots | null, loc?: string): SwitchRegion;
        slot(anchor: Node, slots: Slots | null, name: string, params: object | null, fallback: BlockBuilder | null, loc?: string): StaticRegion;
        call(anchor: Node, name: string, component: unknown, slots: Slots | null, params: object, loc?: string): StaticRegion;
        props: typeof spreadProps;
    };
}

declare module "trame/internal/runtime/regions" {
    /**
     * Régions dynamiques du DOM.
     *
     * Une région est un emplacement repéré par un nœud d'ancrage (texte vide) : son contenu est inséré
     * juste avant. Elle gère un ou plusieurs « items » (blocs construits sous leur propre scope).
     *
     * Quand une région est déjà affichée (scope vivant) et qu'un nouveau contenu doit apparaître,
     * celui-ci est préparé hors du DOM sous une frontière d'attente locale : il n'est inséré qu'une
     * fois toutes ses données chargées. Pour un t-if, l'ancien contenu reste affiché en attendant.
     */
    import { Effect } from "trame/internal/reactivity/core";
    import { Boundary, Owner } from "trame/internal/reactivity/owner";
    import { Signal } from "trame/internal/reactivity/core";
    export abstract class Region {
        readonly anchor: Node;
        constructor(anchor: Node);
        /** Premier nœud DOM actuellement occupé par la région (l'ancre si elle est vide). */
        abstract firstNode(): Node;
    }
    export type Root = Node | Region;
    export type BlockBuilder = () => Root[];
    export interface Item {
        roots: Root[];
        owner: Owner;
        /** Nœud temporaire occupant la place de l'item tant qu'il n'est pas prêt. */
        placeholder: Node | null;
    }
    export function firstOf(root: Root): Node;
    export function itemFirst(item: Item): Node;
    export function itemLast(item: Item): Node;
    /** Déplace (ou insère) les nœuds de `first` à `last` avant `before`. */
    export function moveRange(first: Node, last: Node, parent: Node, before: Node | null): void;
    export function removeRange(first: Node, last: Node): void;
    export function insertItem(item: Item, parent: Node, before: Node | null): void;
    export function removeItem(item: Item): void;
    /** Construit un bloc sous un nouveau scope enfant de `parent`. */
    export function buildItem(parent: Owner, build: BlockBuilder, boundary?: Boundary): Item;
    /** Effet de rendu (exécuté immédiatement). `loc` : localisation dans le template (erreurs). */
    export function renderEffect(fn: () => void, loc?: string, priority?: number): Effect;
    export class StaticRegion extends Region {
        private item;
        /**
         * @param shareOwner  le contenu est seul dans un bloc qui a déjà son propre scope (ligne, branche,
         *                    slot) : il l'utilise directement au lieu d'en créer un de plus.
         */
        constructor(anchor: Node, build: BlockBuilder, loc?: string, shareOwner?: boolean);
        firstNode(): Node;
    }
    /**
     * Pendant un remplacement, le contenu sortant reste affiché mais gelé (Owner.suspend) : il ne suit
     * plus l'état du parent (props, ressources) et reprend vie si le remplacement est annulé. Le choix de
     * la clé s'exécute avec la priorité des ressources, pour geler le contenu sortant avant que ses
     * @resource ne se relancent sur le même changement.
     */
    export class SwitchRegion extends Region {
        private readonly builderFor;
        private readonly loc?;
        private current;
        private currentKey;
        private pending;
        private readonly owner;
        constructor(anchor: Node, keyFn: () => unknown, builderFor: (key: unknown) => BlockBuilder | null, loc?: string | undefined);
        firstNode(): Node;
        private update;
        private commitPending;
        private cancelPending;
        private removeCurrent;
    }
    export class ListRegion extends Region {
        private readonly keyFn;
        private readonly rowBuilder;
        /** Le template lit-il `x_index` ? (vrai par défaut : templates précompilés plus anciens). */
        private readonly withIndex;
        private rows;
        private readonly owner;
        /** Sans t-key : clés de remplacement (stables) des 2e, 3e... occurrences d'une même valeur. */
        private duplicates;
        constructor(anchor: Node, listFn: () => unknown, keyFn: ((item: unknown, index: number) => unknown) | null, rowBuilder: (item: Signal<unknown>, index: Signal<number> | null) => Root[], loc?: string, 
        /** Le template lit-il `x_index` ? (vrai par défaut : templates précompilés plus anciens). */
        withIndex?: boolean);
        firstNode(): Node;
        /**
         * Sans t-key, la clé d'une ligne est sa valeur. Une valeur présente plusieurs fois reçoit, pour
         * chaque occurrence supplémentaire, une clé de remplacement stable d'une mise à jour à l'autre.
         */
        private identityKeys;
        private reconcile;
        private createRow;
        private rowInserted;
        private rowReady;
        private removeRow;
    }
    /** Indices (dans `arr`) formant une plus longue sous-suite strictement croissante, en ignorant les -1. */
    export function longestIncreasingSubsequence(arr: ArrayLike<number>): number[];
    export class Markup {
        readonly html: string;
        constructor(html: string);
        toString(): string;
    }
    /** Marque une chaîne comme HTML sûr : t-out l'insérera sans échappement. */
    export function markup(html: string): Markup;
    export class OutRegion extends Region {
        private nodes;
        constructor(anchor: Node, valueFn: () => unknown, loc?: string);
        firstNode(): Node;
        private update;
        private replace;
    }
}

declare module "trame/internal/runtime/template" {
    /**
     * Templates : source XML, extensions (héritage par xpath), compilation paresseuse et mise en cache.
     *
     * Deux façons d'obtenir la fonction de rendu d'un template :
     * - le compiler dans le navigateur, au premier affichage (version complète, trame.js) ;
     * - l'avoir reçu déjà compilé (registerCompiled), par exemple depuis le serveur : c'est la seule
     *   possible avec trame.runtime.js, qui n'inclut pas le compilateur.
     */
    import type { CompileMode } from "trame/internal/compiler/codegen";
    import type { XElement, XNode } from "trame/internal/compiler/xml";
    import type { Slots } from "trame/internal/runtime/component";
    import { helpers } from "trame/internal/runtime/helpers";
    import type { Root } from "trame/internal/runtime/regions";
    export type RenderFunction = (component: unknown, slots: Slots | null, params: object | null) => Root[];
    /** Fabrique d'une fonction de rendu (code généré par le compilateur : `(function ($h) { ... })`). */
    export type RenderFactory = (h: typeof helpers) => RenderFunction;
    /** Fonctions de rendu précompilées d'un template, par mode. */
    export interface CompiledTemplate {
        component?: RenderFactory;
        call?: RenderFactory;
    }
    /** Extension : source XML, ou opérations déjà parsées (fichiers de templates). */
    export type TemplateExtension = string | XElement[];
    /** Compilateur branché par la version complète de Trame (absent de trame.runtime.js). */
    export interface TemplateCompiler {
        nodes(template: Template): XNode[];
        code(template: Template, mode: CompileMode): string;
        registerFile(content: string, path: string): void;
    }
    /** Branche (ou retire, avec null) le compilateur de templates. */
    export function setTemplateCompiler(value: TemplateCompiler | null): void;
    export class Template {
        readonly source: string;
        name: string;
        readonly base: Template | null;
        readonly extensions: TemplateExtension[];
        private readonly cache;
        private precompiled;
        /** Arbre fourni par un fichier de templates (registerTemplates). */
        loader: (() => XNode[]) | null;
        /** Templates dérivés (inheritTemplate) : ils doivent être recompilés quand celui-ci change. */
        private readonly derived;
        constructor(source: string, name?: string, base?: Template | null);
        /** Ajoute une extension (xpath) : les prochains rendus utiliseront la version étendue. */
        extend(extension: TemplateExtension): void;
        /** Oublie la version compilée (la source ou ses extensions ont changé), ainsi que celle des dérivés. */
        invalidate(): void;
        /** Fonctions de rendu reçues déjà compilées. */
        setCompiled(compiled: CompiledTemplate): void;
        /** Arbre XML final (base + extensions). Nécessite le compilateur. */
        getNodes(): XNode[];
        /** Code JS généré (débogage, précompilation). Nécessite le compilateur. */
        getCode(mode?: CompileMode): string;
        getRender(mode: CompileMode): RenderFunction;
    }
    /**
     * Déclare un template inline :
     *   static template = xml`<div>{{ name }}</div>`;
     */
    export function xml(strings: TemplateStringsArray, ...values: unknown[]): Template;
    /** Template nommé existant, ou créé vide s'il n'existe pas encore. */
    export function namedTemplate(name: string): Template;
    export function hasTemplate(name: string): boolean;
    /** Enregistre un template nommé (utilisable par t-call ou par `static template = "nom"`). */
    export function registerTemplate(name: string, source: string | Template): Template;
    /**
     * Enregistre un template déjà compilé (par le serveur, avec compileTemplateFiles) :
     *   registerCompiled("sale.OrderForm", { component: (function ($h) { ... }) });
     */
    export function registerCompiled(name: string, compiled: CompiledTemplate): Template;
    /**
     * Enregistre un fichier de templates (<templates> avec des <t t-name> et des <t t-inherit>),
     * compilés dans le navigateur. Nécessite le compilateur (trame.js).
     */
    export function registerTemplates(content: string, path?: string): void;
    export function getTemplate(name: string): Template;
    type TemplateTarget = Template | string | {
        name: string;
        template?: Template | string;
    };
    /**
     * Étend un template existant (mode « extension ») : toutes ses utilisations voient la modification.
     * L'extension contient des <xpath expr="..." position="..."> (ou des éléments avec position="...").
     */
    export function extendTemplate(target: TemplateTarget, extension: string): void;
    /** Crée un nouveau template à partir d'un autre (mode « primaire »), sans modifier l'original. */
    export function inheritTemplate(base: TemplateTarget, extension: string, name?: string): Template;
    /** Template d'une classe de composant. */
    export function resolveTemplate(Ctor: {
        name: string;
        template?: Template | string;
    }): Template;
}

declare module "trame/internal/testing" {
    /**
     * Utilitaires de test pour les composants Trame (Vitest, Jest... avec un DOM : jsdom, happy-dom).
     *
     *   import { cleanup, click, render, settle } from "trame/testing";
     *
     *   afterEach(cleanup);
     *
     *   test("compteur", async () => {
     *       const { html } = await render(Counter, { props: { start: 1 } });
     *       await click("button");
     *       expect(html()).toBe("<button>2</button>");
     *   });
     */
    import { type Component, type ComponentClass, type MountOptions, type Root } from "trame/internal/index";
    export interface Rendered<C extends Component> {
        /** Élément (attaché au document) dans lequel le composant est monté. */
        fixture: HTMLElement;
        root: Root<C>;
        component: C;
        /** HTML actuel du composant. */
        html(): string;
        /** Démonte le composant et retire le fixture du document. */
        destroy(): void;
    }
    /**
     * Monte un composant dans un élément attaché au document (mode dev activé par défaut).
     * La promesse est résolue une fois le composant affiché, donc ses données chargées.
     */
    export function render<C extends Component>(Ctor: ComponentClass<C>, options?: MountOptions): Promise<Rendered<C>>;
    /** Démonte tous les composants montés avec render() (à appeler dans afterEach). */
    export function cleanup(): void;
    /** Attend que les promesses en cours et les mises à jour du DOM soient traitées. */
    export function settle(rounds?: number): Promise<void>;
    export interface Deferred<T> {
        promise: Promise<T>;
        resolve(value: T): void;
        reject(error: unknown): void;
    }
    /** Promesse contrôlée par le test (pour simuler une réponse serveur au moment voulu). */
    export function deferred<T>(): Deferred<T>;
    export interface WaitForOptions {
        /** Délai maximal en ms (défaut : 1000). */
        timeout?: number;
        /** Intervalle entre deux essais en ms (défaut : 10). */
        interval?: number;
    }
    /**
     * Réessaie `check` jusqu'à ce qu'il ne lève plus d'erreur (ex. un expect), puis renvoie son résultat.
     * Au-delà du délai, relance la dernière erreur.
     */
    export function waitFor<T>(check: () => T | Promise<T>, options?: WaitForOptions): Promise<T>;
    type Target = Element | string;
    /** Trouve un élément (lève une erreur explicite s'il n'existe pas). */
    export function find<E extends Element = HTMLElement>(selector: string, root?: ParentNode): E;
    /** Déclenche un événement (qui remonte), puis attend les mises à jour. */
    export function trigger(target: Target, type: string, init?: EventInit, root?: ParentNode): Promise<void>;
    /** Clic sur un élément, puis attente des mises à jour. */
    export function click(target: Target, root?: ParentNode): Promise<void>;
    /** Saisie dans un champ : modifie sa valeur et déclenche « input » (et « change » si demandé). */
    export function input(target: Target, value: string, options?: {
        change?: boolean;
    }, root?: ParentNode): Promise<void>;
    /** Coche ou décoche une case (ou un bouton radio) et déclenche « change ». */
    export function check(target: Target, checked?: boolean, root?: ParentNode): Promise<void>;
}

declare module "trame" {
    export * from "trame/internal/index";
}

declare module "trame/testing" {
    export * from "trame/internal/testing";
}

declare module "trame/runtime" {
    export * from "trame/internal/index.runtime";
}

declare module "trame/compiler" {
    export * from "trame/internal/compiler/index";
}
