use rquickjs::{Ctx, Function, Result};

pub fn setup_console<'js>(ctx: &Ctx<'js>) -> Result<()> {
    let globals = ctx.globals();

    globals.set(
        "__print",
        Function::new(ctx.clone(), |s: String| {
            println!("{}", s);
            Ok::<(), rquickjs::Error>(())
        })?,
    )?;

    globals.set(
        "__eprint",
        Function::new(ctx.clone(), |s: String| {
            eprintln!("{}", s);
            Ok::<(), rquickjs::Error>(())
        })?,
    )?;

    ctx.eval::<(), _>(
        r#"
        globalThis.console = {
            log:   (...args) => __print(args.map(String).join(' ')),
            info:  (...args) => __print(args.map(String).join(' ')),
            warn:  (...args) => __eprint('[WARN] ' + args.map(String).join(' ')),
            error: (...args) => __eprint('[ERROR] ' + args.map(String).join(' ')),
        };
        "#,
    )?;

    Ok(())
}
