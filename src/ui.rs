//! Shared macOS/Apple-style design tokens for the local HTML surfaces
//! (the ChatGPT widget resources in `mcp` and the OAuth approval page in `http`).
//!
//! `THEME_TOKENS` is a block of CSS custom properties on `:root` with a matching
//! `prefers-color-scheme: dark` override. Embedding it inside a surface's
//! `<style>` gives every page the same palette, radii, and shadows, and makes
//! each one adapt to light/dark automatically.

pub const THEME_TOKENS: &str = r#"
    :root {
      --accent: #007aff; --accent-press: #006edb; --on-accent: #fff;
      --ring: rgba(0, 122, 255, .28);
      --fg: #1d1d1f; --fg-2: #424245; --muted: #6e6e73;
      --bg: #f2f2f7; --card: #ffffff; --card-2: #fbfbfd; --hairline: #d1d1d6; --fill: #ebebf0;
      --ok: #1f8a3f; --ok-soft: #e6f6ea;
      --bad: #c00c0c; --bad-soft: #fde6e6;
      --warn: #b26a00; --warn-soft: #fbecd1;
      --radius: 12px; --radius-sm: 8px;
      --shadow: 0 1px 2px rgba(0,0,0,.04), 0 12px 28px rgba(0,0,0,.07);
      --glass-bg: rgba(255, 255, 255, .6);
      --glass-bg-2: rgba(255, 255, 255, .42);
      --glass-border: rgba(255, 255, 255, .55);
      --glass-blur: 18px;
      --glass-sat: 180%;
      --glass-rim: inset 0 1px 0 rgba(255,255,255,.7), inset 0 -1px 0 rgba(0,0,0,.04);
      --aura: radial-gradient(115% 115% at 18% -10%, #d8e6ff 0%, rgba(216,230,255,0) 52%), radial-gradient(120% 120% at 105% 110%, #ffe0ec 0%, rgba(255,224,236,0) 55%);
    }
    @media (prefers-color-scheme: dark) {
      :root {
        --accent: #0a84ff; --accent-press: #2493ff; --on-accent: #fff;
        --ring: rgba(10, 132, 255, .4);
        --fg: #f5f5f7; --fg-2: #d1d1d6; --muted: #98989d;
        --bg: #000000; --card: #1c1c1e; --card-2: #2c2c2e; --hairline: #38383a; --fill: #2c2c2e;
        --ok: #34c759; --ok-soft: #163524;
        --bad: #ff453a; --bad-soft: #3a1b1a;
        --warn: #ff9f0a; --warn-soft: #33260e;
        --shadow: 0 1px 2px rgba(0,0,0,.5), 0 12px 28px rgba(0,0,0,.55);
        --glass-bg: rgba(30, 30, 32, .52);
        --glass-bg-2: rgba(48, 48, 52, .42);
        --glass-border: rgba(255, 255, 255, .14);
        --glass-blur: 22px;
        --glass-sat: 160%;
        --glass-rim: inset 0 1px 0 rgba(255,255,255,.16), inset 0 -1px 0 rgba(0,0,0,.35);
        --aura: radial-gradient(115% 115% at 18% -10%, #1a2747 0%, rgba(26,39,71,0) 52%), radial-gradient(120% 120% at 105% 110%, #3a1d33 0%, rgba(58,29,51,0) 55%);
      }
    }
"#;
