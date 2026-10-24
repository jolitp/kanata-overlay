# Quickstart: Live Key Highlighting

§1 needs nothing; §2 uses the fake Kanata (agents can run it); §3 is for the user, with
real Kanata and devices.

## 1. Automated

```sh
just check
```

Covers ([data-model.md](data-model.md)): `parse_key_event` on Kanata's real line format
(with and without the timestamp, ANSI already stripped, `Press`/`Release`/`Repeat`,
`BTN_*`), non-matching lines; `DebugKeyTap` consumes every debug line and passes only
key events on; the supervisor's log holds no debug line when a tap is set; `route`
(every row of the filter table); `is_typing_key`, `preferred_name`; the `highlight`
setting round trip, default and leniency.

## 2. With the fake Kanata

A separate settings file with one managed device, config `examples/kanata/minimal.kbd`:

```sh
FAKE_KEYS="KEY_CAPSLOCK:Press KEY_A:Press KEY_A:Release KEY_CAPSLOCK:Release" \
FAKE_TAPHOLD="caps:tap caps:hold" \
cargo run -p kanata-overlay -- --settings "$SP/settings.json" \
  --kanata crates/kanata-client/tests/fake-kanata.sh
```

1. **Highlight (US1)**: Caps and A light and go dark in turn on the board (screenshot while held).
2. **Tap-hold (US2)**: Caps shows "tap", then "hold".
3. **No typing keys**: set Highlight to "No typing keys": Caps still lights, A never does.
4. **Off**: set Highlight to "Off": the fake Kanata restarts without `--debug` (log), nothing lights, and tap-hold results aren't shown either.
5. **Hidden**: hide the overlay: nothing is kept; show it: no stale highlight.
6. **Log window**: open Show log: no `[DEBUG]` line, no key name from `FAKE_KEYS`.
7. **Press to bind (US3)**: in the layout editor, select a key, Press to bind: the next fake press binds it; with Highlight "Off" the button is disabled with the reason.
8. **Privacy (SC-005)**: after the run, `grep -r KEY_A` over the settings folder, `layouts/` and the app's stderr finds nothing new.

## 3. With the user's devices (user)

1. **Defy**: type: each key lights on the Defy's board only; hold several: all light; release all after a minute of fast typing: nothing stays lit (SC-002).
2. **Mouse**: tap Caps quickly: "tap"; hold it: "hold" (10 of 10, SC-003). Side buttons light.
3. **No typing keys** on the Defy: letters never light; Shift, arrows, Esc do.
4. **Press to bind**: in the layout editor, bind an unbound key by pressing it (under 5 s, SC-004); the key still does its normal thing.
5. **Kanata started by hand** (`just kanata`, attached): tap-hold results show; settings says pressed keys need a Kanata started by the overlay.

## Validation log

**Automated (2026-10-23)**: `just check` passes. New tests: `parse_key_event` on
Kanata's line format (timestamped, untimestamped, Press/Release/Repeat, `BTN_*`, junk),
`DebugKeyTap` swallowing every debug/trace line and passing only key events, `route`
(every rule, capture beating the rest), `is_typing_key` / `preferred_name`, the
`highlight` setting (missing, invalid, round trip), and a supervisor test against the
fake Kanata: key events reach the tap in order and the log holds no `[DEBUG]` line or
`KEY_` name.

**Agent, fake Kanata (2026-10-23)**, scripted with a separate settings file:
- §2.1 / §2.2: the board received `["KEY_CAPSLOCK"]`, `["KEY_A","KEY_CAPSLOCK"]`, `[]`
  and tap/hold results `tap`, `hold`, `{}`; a screenshot shows Caps and A lit with the
  hold badge on Caps.
- Latency (C1): a key event reaches the board 0.3–0.5 ms after it is read (trace log,
  timing only). Kanata's own pipe buffering can only be judged with real Kanata (§3).
- §2.3: with "No typing keys" only Caps ever appeared; A never did.
- §2.4: "Off" restarted the fake Kanata (checking → starting → managed) without
  `--debug`; `pressSource` "off"; nothing more arrived.
- §2.6: the log window had no `[DEBUG]` line and no `KEY_` name.
- §2.7: Press to bind bound the selected key to `caps` (the config's spelling of the
  captured `KEY_CAPSLOCK`), as one undo step; with "Off" it was unavailable.
- §2.8 / SC-005: no file in the scratch settings folder and no line of the app's output
  (outside the test's own prints) names a key.

**Changes from the analysis**: a highlight mode change clears held keys (I1); US2's
hold lasts 1.5 s when releases aren't visible (I2). `watching` follows
`overlayVisible` (shown or previewed), not only the show/hide toggle.

**User, real devices (2026-10-24), T028**:
- §3.1 Defy: keys light on the Defy's board; no lag through the pipe, nothing stuck.
  One key stayed dark: the key left of A was bound to `caps` in `defy.json` but sends
  Esc (both have the same tap-hold in `defy.kbd`, so its label looked right). Rebound
  with Press to bind; it lights since. Passed.
- §3.2 tap/hold: green "tap" and yellow "hold" on the mouse's Caps and the Defy's Esc,
  which enters `nav`. Passed.
- §3.4 Press to bind: used for the fix above. Passed.
- U1: Kanata's CPU use doesn't move while typing with `--debug`. Passed.
- Found: after a click, a settings dropdown kept keyboard focus, so typing `a`, `n` or
  `o` on the Defy picked another Highlight option (All / No typing / Off). Fixed: every
  dropdown in the settings window and the layout editor takes focus by Tab only.
- §3.3 "No typing keys": only Shift, Ctrl and other non-typing keys light while typing.
  Passed. After the fix, clicking a dropdown no longer captures typing; Tab still
  reaches every dropdown, starting from the top field.
