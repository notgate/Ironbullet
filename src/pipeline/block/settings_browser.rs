use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserOpenSettings {
    pub headless: bool,
    pub browser_type: String,
    pub proxy: String,
    pub extra_args: String,
}

impl Default for BrowserOpenSettings {
    fn default() -> Self {
        Self {
            headless: true,
            browser_type: "chromium".into(),
            proxy: String::new(),
            extra_args: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigateToSettings {
    pub url: String,
    pub wait_until: String,
    pub timeout_ms: u64,
    #[serde(default)]
    pub custom_cookies: String,
}

impl Default for NavigateToSettings {
    fn default() -> Self {
        Self {
            url: String::new(),
            wait_until: "load".into(),
            timeout_ms: 30000,
            custom_cookies: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickElementSettings {
    pub selector: String,
    pub wait_for_navigation: bool,
    pub timeout_ms: u64,
}

impl Default for ClickElementSettings {
    fn default() -> Self {
        Self {
            selector: String::new(),
            wait_for_navigation: false,
            timeout_ms: 5000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeTextSettings {
    pub selector: String,
    pub text: String,
    pub clear_first: bool,
    pub delay_ms: u64,
}

impl Default for TypeTextSettings {
    fn default() -> Self {
        Self {
            selector: String::new(),
            text: String::new(),
            clear_first: true,
            delay_ms: 50,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitForElementSettings {
    pub selector: String,
    pub state: String,
    pub timeout_ms: u64,
}

impl Default for WaitForElementSettings {
    fn default() -> Self {
        Self {
            selector: String::new(),
            state: "visible".into(),
            timeout_ms: 10000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetElementTextSettings {
    pub selector: String,
    pub attribute: String,
    pub output_var: String,
    pub capture: bool,
}

impl Default for GetElementTextSettings {
    fn default() -> Self {
        Self {
            selector: String::new(),
            attribute: String::new(),
            output_var: "ELEMENT_TEXT".into(),
            capture: false,
        }
    }
}

/// Reads the live page DOM, or the HTML for a CSS-selected element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetDomSettings {
    /// Empty means the full current page; otherwise this CSS selector is queried.
    #[serde(default)]
    pub selector: String,
    /// Selected elements return outer HTML by default; false returns inner HTML.
    #[serde(default = "default_outer_html")]
    pub outer_html: bool,
    pub output_var: String,
    pub capture: bool,
}

fn default_outer_html() -> bool {
    true
}

impl Default for GetDomSettings {
    fn default() -> Self {
        Self {
            selector: String::new(),
            outer_html: true,
            output_var: "DOM".into(),
            capture: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotSettings {
    pub full_page: bool,
    pub selector: String,
    pub output_var: String,
}

impl Default for ScreenshotSettings {
    fn default() -> Self {
        Self {
            full_page: false,
            selector: String::new(),
            output_var: "SCREENSHOT_B64".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteJsSettings {
    pub code: String,
    pub output_var: String,
    pub capture: bool,
}

impl Default for ExecuteJsSettings {
    fn default() -> Self {
        Self {
            code: String::new(),
            output_var: "JS_RESULT".into(),
            capture: false,
        }
    }
}
