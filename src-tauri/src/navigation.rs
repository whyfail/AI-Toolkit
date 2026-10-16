use tauri::{App, Runtime, Url, WebviewWindowBuilder};
use tauri_plugin_shell::ShellExt;

fn local_navigation(url: &Url, dev_url: Option<&Url>, use_https: bool) -> bool {
    if !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    if let Some(dev) = dev_url {
        if url.origin() == dev.origin() {
            return true;
        }
    }
    url.port().is_none()
        && ((url.scheme() == "tauri" && url.host_str() == Some("localhost"))
            || (url.scheme() == if use_https { "https" } else { "http" }
                && url.host_str() == Some("tauri.localhost")))
}

fn open_external<R: Runtime>(app: &tauri::AppHandle<R>, url: &Url) {
    if matches!(url.scheme(), "http" | "https")
        && url.username().is_empty()
        && url.password().is_none()
    {
        #[allow(deprecated)]
        if let Err(error) = app.shell().open(url.as_str(), None) {
            log::warn!("打开文档外链失败: {error}");
        }
    }
}

pub fn create_main_window<R: Runtime>(app: &App<R>) -> anyhow::Result<()> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "main")
        .ok_or_else(|| anyhow::anyhow!("缺少主窗口配置"))?;
    let dev_url = tauri::is_dev()
        .then(|| app.config().build.dev_url.clone())
        .flatten();
    let use_https = config.use_https_scheme;
    let navigation_app = app.handle().clone();
    let popup_app = app.handle().clone();
    WebviewWindowBuilder::from_config(app, config)?
        .on_navigation(move |url| {
            if local_navigation(url, dev_url.as_ref(), use_https) {
                return true;
            }
            open_external(&navigation_app, url);
            false
        })
        .on_new_window(move |url, _| {
            open_external(&popup_app, &url);
            tauri::webview::NewWindowResponse::Deny
        })
        .build()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_app_origin_is_required_and_dev_origin_is_scoped() {
        for allowed in [
            "tauri://localhost/index.html#section",
            "http://tauri.localhost/",
        ] {
            assert!(local_navigation(&Url::parse(allowed).unwrap(), None, false));
        }
        for denied in [
            "https://tauri.example.test/",
            "http://tauri.localhost.example.test/",
            "tauri://other/",
            "file:///tmp/test",
            "javascript:alert(1)",
            "http://localhost:1420/",
            "http://tauri.localhost:9999/",
            "http://user@tauri.localhost/",
        ] {
            assert!(
                !local_navigation(&Url::parse(denied).unwrap(), None, false),
                "{denied}"
            );
        }
        let dev = Url::parse("http://localhost:1420").unwrap();
        assert!(local_navigation(
            &Url::parse("http://localhost:1420/page").unwrap(),
            Some(&dev),
            false
        ));
        assert!(!local_navigation(
            &Url::parse("http://localhost:1421/").unwrap(),
            Some(&dev),
            false
        ));
        assert!(!local_navigation(
            &Url::parse("http://localhost.example.test:1420/").unwrap(),
            Some(&dev),
            false
        ));
    }
}
