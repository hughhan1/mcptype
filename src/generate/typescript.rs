//! The `@holon-sh/apps`-style package: `index.*` for Node, `browser.*` for browsers.
use super::{Caller, GenerateRequest, Target, declarations};
use crate::{Contract, Error, Language, Resource, Tool};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

const BINDING_TYPES: &str = r#"export interface EndpointBinding { endpoint: URL | string; authProvider?: AuthProvider; installationUID?: string }
export interface HostBinding { host: { callServerTool(params: { name: string; arguments?: Record<string, unknown> }, options?: { signal?: AbortSignal }): Promise<unknown>; readServerResource(params: { uri: string }, options?: { signal?: AbortSignal }): Promise<unknown> } }
export interface CreateOptions { validator?: (schema: object) => (value: unknown) => boolean }
export declare function createApps<K extends keyof Apps>(bindings: { [A in K]: EndpointBinding | HostBinding }, options?: CreateOptions): Pick<Apps,K>;
"#;

const EXPOSE_TYPES: &str = r#"export interface ExposeOptions { signal?: AbortSignal; execute?: (name: string, args: Record<string, unknown>) => Promise<unknown> }
export declare function exposeTools(app: Apps[keyof Apps], options?: ExposeOptions): Promise<boolean>;
export type PageResult<I> = { readonly value: I; readonly issues?: undefined } | { readonly issues: ReadonlyArray<{ readonly message: string }> };
export interface PageSchema<I = unknown> { readonly "~standard": { readonly version: 1; readonly vendor: string; readonly types?: { readonly input: unknown; readonly output: I }; readonly validate: (value: unknown) => PageResult<I> | Promise<PageResult<I>>; readonly jsonSchema: { readonly input: (options: { target: "draft-2020-12" }) => object } } }
export type PageInput<S> = S extends PageSchema<infer I> ? I : never;
export interface PageAnnotations { title?: string; readOnlyHint?: boolean; destructiveHint?: boolean; idempotentHint?: boolean; openWorldHint?: boolean }
export interface PageTool<S extends PageSchema = PageSchema> { description: string; inputSchema: S; annotations?: PageAnnotations; execute: (args: PageInput<S>) => unknown }
export interface PageHost { registerTool(name: string, config: { description: string; inputSchema: PageSchema; annotations?: PageAnnotations }, callback: (args: any, extra: any) => any): { remove(): void } }
export declare function registerPageTools<S extends { [name: string]: PageSchema }>(tools: { [K in keyof S]: PageTool<S[K]> }, options?: { view?: PageHost; signal?: AbortSignal }): Promise<boolean>;
"#;

/// Resources compiled as tools named by URI, so `Outputs[alias][uri]` is the resource's contents.
fn resource_contracts(contracts: &BTreeMap<String, Contract>) -> BTreeMap<String, Contract> {
    let input = json!({"type": "object", "additionalProperties": false});
    let input = input.as_object().unwrap();
    contracts
        .iter()
        .map(|(alias, contract)| {
            let tools = contract
                .resources
                .iter()
                .map(|(uri, Resource { output })| {
                    let tool = Tool {
                        input: input.clone(),
                        output: Some(output.clone()),
                    };
                    (uri.clone(), tool)
                })
                .collect();
            let contract = Contract {
                tools,
                resources: BTreeMap::new(),
            };
            (alias.clone(), contract)
        })
        .collect()
}

pub(super) fn generate(
    request: &GenerateRequest,
    caller: &Caller,
) -> Result<BTreeMap<&'static str, String>, Error> {
    let contracts = &request.contracts;
    let tools = declarations(Language::TypeScript, contracts)?.declarations;
    let resources =
        declarations(Language::TypeScript, &resource_contracts(contracts))?.declarations;
    let protocol = request.protocol.as_deref().unwrap_or_default();
    let identity = serde_json::to_string(caller)?;
    let contracts = serde_json::to_string(contracts)?;
    let untyped = untyped(request)?;
    let types = |resources: &str| {
        format!(
            "import type {{ Outputs as Resources }} from \"./{resources}.js\";\n{protocol}\nexport type {{ Resources }};\n{tools}\nexport type Apps = {{ readonly [A in keyof Inputs]: ToolClient<Inputs[A], Outputs[A], Resources[A]> }}{untyped};\n"
        )
    };
    let bound = format!(
        "export function createApps(bindings,options){{return bindApps({identity},{contracts},bindings,options)}}\n"
    );
    let package = package(&request.package_fields)?;
    Ok(match request.target {
        Target::Browser => BTreeMap::from([
            (
                "browser.js",
                "import { bindApps } from \"./browser-runtime.mjs\";\nexport {ToolCallError, exposeTools, registerPageTools} from \"./browser-runtime.mjs\";\n".to_owned()
                    + &bound,
            ),
            (
                "browser.d.ts",
                types("browser-resources") + BINDING_TYPES + EXPOSE_TYPES,
            ),
            ("browser-resources.d.ts", resources),
            ("package.json", package),
        ]),
        _ => BTreeMap::from([
            (
                "index.js",
                format!(
                    "import {{ createApps as create, bindApps }} from \"./runtime.mjs\";\nexport {{ ToolCallError }} from \"./runtime.mjs\";\nexport const apps = create({identity}, {contracts});\n{bound}"
                ),
            ),
            (
                "index.d.ts",
                types("resources") + "export declare const apps: Apps;\n" + BINDING_TYPES,
            ),
            ("resources.d.ts", resources),
            ("package.json", package),
        ]),
    })
}

/// Members for aliases without a contract: `callTool` and `listTools` by any name, no `tools` or `resources`.
fn untyped(request: &GenerateRequest) -> Result<String, Error> {
    let aliases = request
        .untyped()
        .map(|alias| {
            Ok(format!(
                " readonly {}: UntypedClient;",
                serde_json::to_string(alias)?
            ))
        })
        .collect::<Result<String, Error>>()?;
    Ok(if aliases.is_empty() {
        aliases
    } else {
        format!(
            " & {{{aliases} }};\nexport type UntypedClient = Omit<ToolClient<Record<string, Record<string, unknown>>, Record<never, never>, Record<never, never>>, \"tools\"> & {{ readonly tools: Record<never, never> }}"
        )
    })
}

/// `package.json` with two-space indentation and the extra fields between `type` and `exports`.
fn package(fields: &Map<String, Value>) -> Result<String, Error> {
    let mut text = String::from(
        "{\n  \"name\": \"@holon-sh/apps\",\n  \"version\": \"0.0.0\",\n  \"private\": true,\n  \"type\": \"module\",\n",
    );
    for (name, value) in fields {
        let value = serde_json::to_string_pretty(value)?.replace('\n', "\n  ");
        text += &format!("  {}: {value},\n", serde_json::to_string(name)?);
    }
    text += r#"  "exports": {
    ".": {
      "types": "./index.d.ts",
      "import": "./index.js"
    },
    "./browser": {
      "types": "./browser.d.ts",
      "import": "./browser.js"
    }
  }
}
"#;
    Ok(text)
}
