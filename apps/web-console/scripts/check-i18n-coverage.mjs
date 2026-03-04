#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const LOCALES_ROOT = path.resolve(process.cwd(), "src/i18n/locales");
const BASELINE_LOCALE = "en-US";
const NAMESPACE_FILE = "common.json";

const localeDirs = fs
  .readdirSync(LOCALES_ROOT, { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name)
  .sort((left, right) => left.localeCompare(right));

if (!localeDirs.includes(BASELINE_LOCALE)) {
  console.error(`FAIL: baseline locale '${BASELINE_LOCALE}' is missing in ${LOCALES_ROOT}`);
  process.exit(1);
}

const localeBundles = new Map();
for (const locale of localeDirs) {
  const filePath = path.join(LOCALES_ROOT, locale, NAMESPACE_FILE);
  if (!fs.existsSync(filePath)) {
    console.error(`FAIL: locale '${locale}' missing namespace file ${NAMESPACE_FILE}`);
    process.exit(1);
  }
  const payload = JSON.parse(fs.readFileSync(filePath, "utf8"));
  localeBundles.set(locale, payload);
}

const baselineKeys = new Set(collectLeafKeys(localeBundles.get(BASELINE_LOCALE)));
if (baselineKeys.size === 0) {
  console.error(`FAIL: baseline locale '${BASELINE_LOCALE}' has no translation leaf keys`);
  process.exit(1);
}

let hasFailure = false;
for (const locale of localeDirs) {
  const keys = new Set(collectLeafKeys(localeBundles.get(locale)));
  const missing = setDiff(baselineKeys, keys);
  const extra = setDiff(keys, baselineKeys);

  if (missing.length === 0 && extra.length === 0) {
    console.log(`PASS: ${locale} key coverage matches ${BASELINE_LOCALE} (${keys.size} keys)`);
    continue;
  }

  hasFailure = true;
  console.error(`FAIL: ${locale} key coverage mismatch`);
  if (missing.length > 0) {
    console.error(`  missing keys (${missing.length}): ${missing.slice(0, 20).join(", ")}`);
  }
  if (extra.length > 0) {
    console.error(`  extra keys (${extra.length}): ${extra.slice(0, 20).join(", ")}`);
  }
}

if (hasFailure) {
  process.exit(1);
}

console.log(`i18n coverage check passed for locales: ${localeDirs.join(", ")}`);

function collectLeafKeys(value, prefix = "") {
  const result = [];
  walk(value, prefix, result);
  return result.filter((key) => key.length > 0).sort((left, right) => left.localeCompare(right));
}

function walk(value, prefix, result) {
  if (Array.isArray(value) || value === null || typeof value !== "object") {
    result.push(prefix);
    return;
  }

  const entries = Object.entries(value);
  if (entries.length === 0) {
    result.push(prefix);
    return;
  }

  for (const [key, child] of entries) {
    const next = prefix.length > 0 ? `${prefix}.${key}` : key;
    walk(child, next, result);
  }
}

function setDiff(leftSet, rightSet) {
  const values = [];
  for (const item of leftSet) {
    if (!rightSet.has(item)) {
      values.push(item);
    }
  }
  values.sort((left, right) => left.localeCompare(right));
  return values;
}
