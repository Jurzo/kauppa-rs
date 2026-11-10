-- 0009_user_language — each user's chosen language (MB-0035).
-- Resolution: RR-0023 sub-answer 3, see
-- .agentic/resolutions/RR-0023-translations.md.
-- Intent: the language a signed-in user chose with the switcher, so the
-- choice follows them across sessions and devices. NULL means "not chosen":
-- the `kauppa_lang` cookie, then `Accept-Language`, then English decide.
--
-- Nullable, no default and no CHECK: core validates on write
-- (`Language::parse`) and reads an unknown value as NULL, so a third
-- language is a catalog file, not a table rebuild. Purely additive: existing
-- rows read NULL.

ALTER TABLE users ADD COLUMN language TEXT;
