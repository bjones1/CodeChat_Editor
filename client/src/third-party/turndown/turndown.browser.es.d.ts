// `turndown.browser.es.d.ts` - TypeScript ignores for the Turndown library
// ========================================================================
//
// This suppress type errors when using the Turndown library.
declare class TurndownService {
    constructor(options: any);
    use(_: Function|Array<Function>): any;
    turndown(_: string|HTMLElement): string;
    next(_: string|HTMLElement): string;
    last(_: string|HTMLElement): string;
    options: {[name: string]: any}
}
export default TurndownService;
