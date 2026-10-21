use super::{
    GenerateRequest, compile_contracts, compile_resources, package_json, put, runtime_text,
};
use crate::{Error, Language};
use std::{collections::BTreeMap, sync::atomic::AtomicBool};

fn resource_import(name: &str) -> String {
    format!("import type {{ Outputs as Resources }} from \"./{name}.js\";\n")
}

fn local_declarations(declarations: &str) -> String {
    declarations.to_owned()
}

fn declarations(
    request: &GenerateRequest,
    cancelled: &AtomicBool,
) -> Result<(String, String), Error> {
    let language = match request.target {
        super::Target::Node | super::Target::Browser => Language::TypeScript,
        _ => unreachable!(),
    };
    let tools = compile_contracts(&request.contracts, language, cancelled)?;
    let resources = compile_resources(&request.contracts, cancelled)?;
    Ok((
        local_declarations(&tools.declarations),
        local_declarations(&resources.declarations),
    ))
}

fn common_types(
    protocol: &str,
    declarations: &str,
    resource_name: &str,
    include_resources: bool,
    request: &GenerateRequest,
) -> String {
    let resource_type = if include_resources {
        "export type { Resources };\n"
    } else {
        ""
    };
    let requires = request
        .requires
        .keys()
        .map(|name| {
            format!(
                "  readonly {}: unknown;\n",
                serde_json::to_string(name).unwrap()
            )
        })
        .collect::<String>();
    format!(
        "{}{}\n{}{}\nexport interface Requires {{\n{requires}}}\nexport interface UntypedDependency {{ callTool(request: CallMetadata & {{ name: string; arguments?: Record<string, unknown> }}, options?: CallOptions): Promise<ToolResult<unknown>>; }}\nexport type Apps = {{ readonly [A in keyof Inputs]: ToolClient<Inputs[A], Outputs[A], Resources[A]> }} & {{ readonly [A in Exclude<keyof Requires, keyof Inputs>]: UntypedDependency }};\n",
        if include_resources {
            resource_import(resource_name)
        } else {
            String::new()
        },
        protocol,
        resource_type,
        declarations
    )
}

pub(crate) fn generate(
    request: &GenerateRequest,
    runtime: &BTreeMap<String, Vec<u8>>,
    cancelled: &AtomicBool,
) -> Result<BTreeMap<String, Vec<u8>>, Error> {
    let protocol = runtime_text(runtime, "protocol.d.ts")?;
    let (declarations, resource_declarations) = declarations(request, cancelled)?;
    let mut files = BTreeMap::new();
    match request.target {
        super::Target::Node => {
            let identity = serde_json::json!({
                "name": request.identity.name,
                "version": request.identity.version,
                "requires": request.requires,
            });
            put(
                &mut files,
                "index.js",
                format!(
                    "import {{ createApps as create, bindApps }} from \"./runtime.mjs\";\nexport {{ ToolCallError, setDefaultBinding }} from \"./runtime.mjs\";\nexport const apps = create({}, {});\nexport function createApps(bindings, options){{return bindApps({}, {}, bindings, options)}}\n",
                    serde_json::to_string(&identity)?,
                    serde_json::to_string(&request.contracts)?,
                    serde_json::to_string(&identity)?,
                    serde_json::to_string(&request.contracts)?,
                ),
            );
            let binding_types = "export interface EndpointBinding { endpoint: URL | string; authProvider?: AuthProvider; fetch?: typeof fetch; installationUID?: string; scoped?: boolean }\nexport interface HostBinding { host: { callServerTool(params: { name: string; arguments?: Record<string, unknown> }, options?: { signal?: AbortSignal }): Promise<unknown>; readServerResource(params: { uri: string }, options?: { signal?: AbortSignal }): Promise<unknown> } }\nexport interface CreateAppsOptions { validator?: (schema: Record<string, unknown>) => (value: unknown) => boolean }\nexport declare function setDefaultBinding(resolve: (app: string) => EndpointBinding | HostBinding | undefined): void;\nexport declare function createApps<K extends keyof Apps>(bindings: { [A in K]: EndpointBinding | HostBinding }, options?: CreateAppsOptions): Pick<Apps,K>;\n";
            put(
                &mut files,
                "index.d.ts",
                common_types(&protocol, &declarations, "resources", true, request)
                    + "export declare const apps: Apps;\n"
                    + binding_types,
            );
            put(&mut files, "resources.d.ts", resource_declarations);
            put(
                &mut files,
                "runtime.mjs",
                runtime_text(runtime, "runtime.mjs")?,
            );
            put(
                &mut files,
                "THIRD-PARTY-NOTICES.txt",
                runtime_text(runtime, "THIRD-PARTY-NOTICES.txt")?,
            );
            files.insert("package.json".into(), package_json(request)?);
        }
        super::Target::Browser => {
            let identity = serde_json::json!({
                "name": request.identity.name,
                "version": request.identity.version,
                "requires": request.requires,
            });
            let binding_types = "export interface EndpointBinding { endpoint: URL | string; authProvider?: AuthProvider; fetch?: typeof fetch; installationUID?: string; scoped?: boolean }\nexport interface HostBinding { host: { callServerTool(params: { name: string; arguments?: Record<string, unknown> }, options?: { signal?: AbortSignal }): Promise<unknown>; readServerResource(params: { uri: string }, options?: { signal?: AbortSignal }): Promise<unknown> } }\nexport interface CreateAppsOptions { validator?: (schema: Record<string, unknown>) => (value: unknown) => boolean }\nexport declare function setDefaultBinding(resolve: (app: string) => EndpointBinding | HostBinding | undefined): void;\nexport declare function createApps<K extends keyof Apps>(bindings: { [A in K]: EndpointBinding | HostBinding }, options?: CreateAppsOptions): Pick<Apps,K>;\nexport interface ExposeOptions { signal?: AbortSignal; execute?: (name: string, args: Record<string, unknown>) => Promise<unknown> }\nexport declare function exposeTools(app: Apps[keyof Apps], options?: ExposeOptions): Promise<boolean>;\n";
            put(
                &mut files,
                "browser.js",
                format!(
                    "import {{ bindApps }} from \"./browser-runtime.mjs\";\nexport {{ToolCallError, exposeTools, setDefaultBinding}} from \"./browser-runtime.mjs\";\nexport function createApps(bindings, options){{return bindApps({}, {}, bindings, options)}}\n",
                    serde_json::to_string(&identity)?,
                    serde_json::to_string(&request.contracts)?,
                ),
            );
            put(
                &mut files,
                "browser.d.ts",
                common_types(&protocol, &declarations, "browser-resources", true, request)
                    + binding_types,
            );
            put(&mut files, "browser-resources.d.ts", resource_declarations);
            put(
                &mut files,
                "browser-runtime.mjs",
                runtime_text(runtime, "browser-runtime.mjs")?,
            );
            if let Some(validator) = runtime.get("validate.mjs") {
                files.insert("validate.mjs".into(), validator.clone());
            }
            put(
                &mut files,
                "THIRD-PARTY-NOTICES.txt",
                runtime_text(runtime, "THIRD-PARTY-NOTICES.txt")?,
            );
        }
        _ => unreachable!(),
    }
    Ok(files)
}
