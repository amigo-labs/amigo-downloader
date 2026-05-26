# Release-TODO — manuelle Schritte zu v1.0

Diese Datei sammelt ausschließlich die **manuell durchzuführenden** Schritte
auf dem Weg zu einem v1.0-Release — Dinge, die ein Maintainer mit Zugang zu
Secrets, bezahlten Accounts oder lokalen Signing-Keys erledigen muss. Reine
Code-Aufgaben (Marketplace-UI, Hoster-Plugins, Rate-Limit, OpenAPI-Codegen,
Web-UI-Tests, CHANGELOG-Backfill, Rustdoc) gehören **nicht** hierher und
laufen über eigene PRs.

Stand: v0.1.0 ist getaggt (`236c2a0`), Auto-Release-on-merge ist aktiv, und
4/4 CRITICAL + 11/17 HIGH aus `docs/specs/audit-2026-04-25.md` sind gemergt.

## Signing & Secrets

- [ ] Tauri-Signing-Key erzeugen (`tauri signer generate`, lokal/offline).
- [ ] `TAURI_SIGNING_PRIVATE_KEY` und `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
      als GitHub-Repo-Secrets hinterlegen (werden in
      `.github/workflows/release-build.yml` bereits gelesen).
- [ ] Pubkey in `tauri/tauri.conf.json` gegen den neuen Key abgleichen.
      Danach kann der Code-Schritt `"active": true` (Updater) gesetzt werden.
- [ ] Registry-Ed25519-**Private**-Key erzeugen und offline sichern;
      sicherstellen, dass `AMIGO_REGISTRY_PUBKEY_HEX` (Build-Arg in
      `.github/workflows/release-build.yml` bzw. Docker-Build) zum
      Public-Key passt.
- [ ] Signier-Prozess für den Plugin-Registry-Index etablieren und
      dokumentieren (wer signiert beim Veröffentlichen neuer Plugins?).

## Accounts & Test-Fixtures (für Verifikation nötig)

- [ ] Bezahlte Test-Accounts für Premiumize und AllDebrid besorgen, um die
      Plugins unter `plugins/multi-hosters/*` end-to-end zu verifizieren.
- [ ] Live-YouTube-Fixtures aufnehmen (Player-JS + `signatureCipher`-Sample)
      für den Fallback-Test zu Audit-Finding #15.

## Veröffentlichung & Distribution

- [ ] Entscheiden, ob `@amigo/plugin-sdk` öffentlich auf npm publiziert wird.
      Falls ja: npm-Org/Account + `NPM_TOKEN`-Secret anlegen und
      `"private": true` in `plugin-sdk/package.json` entfernen.
- [ ] release-please-Release-PR für v1.0 reviewen und mergen (Tag-Cut).
- [ ] Erstes signiertes Desktop-Bundle manuell auf je einer Plattform
      (Linux / macOS / Windows) installieren und das Auto-Update von einer
      heruntergestuften Version aus verifizieren.

## Produkt-Entscheidungen (menschliches Urteil)

- [ ] `docs/specs/docker-security.md`: umsetzen oder begründet für v1.0
      zurückstellen.
- [ ] `docs/specs/tauri-workspace-integration.md`: Restscope festlegen,
      nachdem der Tauri-Updater scharfgeschaltet ist.
