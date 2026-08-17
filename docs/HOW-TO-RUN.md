# How to Run StockScanner — Simple Guide (No Tech Skills Needed)

This guide is for someone who has **never used a command line** and just wants to open the
app, type a stock symbol, and read the result. Follow the steps for your computer.

There is **nothing to install** — you download one file and double-click it.

> What the app does: you type a stock symbol (like `BSX`), and it tells you whether the
> strategy sees it as a Buy / Watch / Wait / Avoid, with the price levels. It is a research
> helper, **not** financial advice and **not** a guarantee — it can be wrong.

---

## Step 1 — Download the app

1. Open this link in your web browser:
   **https://github.com/anthonyshafer/autoparts/releases**
2. You'll see a list. Click the newest one at the top (the **Latest** release).
3. Under **"Assets,"** click to download the file for your computer:
   - **Windows** → `StockScanner.exe`
   - **Mac** → `StockScanner.dmg` (the installer — easiest)
4. Wait for it to finish downloading (check your browser's Downloads, usually bottom-left,
   or your **Downloads** folder).

> **Don't see a release / the list is empty?** The app hasn't been "published" yet. Send
> the owner of the project this one line: *"Please run `git tag v1.0 && git push --tags` so
> the Releases page has a download."* Then come back to this step.

---

## Step 2 — Open the app

### On Windows

1. Find `StockScanner.exe` in your **Downloads** folder and **double-click** it.
2. Windows may show a blue box that says **"Windows protected your PC."** This is normal for
   new apps. Click the small text **"More info,"** then the button **"Run anyway."**
3. A window titled **StockScanner** opens. You're ready — go to Step 3.

### On Mac

1. Find `StockScanner.dmg` in your **Downloads** folder and **double-click** it. A window
   opens showing the **StockScanner** app and an **Applications** shortcut.
2. **Drag** the StockScanner icon onto the **Applications** folder in that window. (This
   installs it.) Then close the window.
3. Open your **Applications** folder, **right-click** (or Control-click) StockScanner, and
   choose **Open**.
4. A box may say the app is from an unidentified developer. Click **Open** again. (You only
   have to do this the first time.)
5. The **StockScanner** window opens. Go to Step 3.

> Mac still refusing to open it? Skip to **Troubleshooting** at the bottom.

---

## Step 3 — Use it

1. In the box at the top, type a stock symbol. Examples: `BSX`, `SOFI`, `UBER`, `PFE`.
   (A symbol is the short code for a company — Boston Scientific is `BSX`, Pfizer is `PFE`.)
2. Leave **Timeframe** on **weekly** (that's the normal setting).
3. Click the **Scan** button.
4. Wait a few seconds (it's fetching live prices from the internet — you must be online).
5. Read the result in the big box.

That's it. To check another stock, change the symbol and click **Scan** again.

---

## Step 4 — What the result means

The most important line is the **verdict** at the top:

| You see | It means |
|---|---|
| **BUY** | The strategy's conditions are met right now. |
| **WATCH** | Close, but not yet — wait for the price to move above a level it names. |
| **WAIT** | Priced around fair value; poor risk/reward right now. |
| **AVOID** | The strategy has no setup here (usually the price is above its long-term average). |

Other lines you'll see:
- **Entry** — roughly where you'd buy.
- **Take-profit** — where the strategy would aim to sell.
- **Stop-loss** — the safety exit if it goes the wrong way.
- **Sizing** — how many shares a chosen dollar amount would buy.

Remember: this is a **rules-based helper, not advice, and not always right.** It's one input,
not a decision. Never risk money you can't afford to lose.

---

## Troubleshooting

**"It just closed / nothing happened."**
Give it a few seconds first — the first launch is slow. If it still won't open, re-download
the file (the download may have been incomplete) and try Step 2 again.

**"It says error / no data."**
- Make sure you're **connected to the internet.**
- Check the symbol is spelled right (e.g. `BSX`, not `BSTON`). If unsure, search
  "[company name] stock symbol" online.

**Windows won't let me run it (SmartScreen keeps blocking).**
Click **"More info"** → **"Run anyway."** If your workplace computer blocks it entirely,
you may need permission from your IT department — that's a security setting, not a problem
with the app.

**Mac says "cannot be opened because it is from an unidentified developer."**
Right-click the app → **Open** → **Open** (not a normal double-click). If it still blocks,
open **System Settings → Privacy & Security**, scroll down, and click **"Open Anyway"** next
to the StockScanner message.

**I still can't get it to work.**
Send the project owner: the name of your computer (Windows or Mac), and exactly what the
error box says. That's enough for them to help.

---

*This app is a research tool, not licensed financial advice. It fetches public market data
and applies a fixed set of rules. It can be wrong. You are responsible for your own trades.*
