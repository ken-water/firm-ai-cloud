#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const appPath = path.resolve(process.cwd(), "src/App.tsx");
const localePath = path.resolve(process.cwd(), "src/i18n/locales/en-US/common.json");

const source = fs.readFileSync(appPath, "utf8");
const locale = JSON.parse(fs.readFileSync(localePath, "utf8"));

const sourceChecks = [
  {
    name: "Role write policy (admin/operator) is present",
    pattern: /const canWriteCmdb = roleSet\.has\("admin"\) \|\| roleSet\.has\("operator"\);/
  },
  {
    name: "Role admin policy is present",
    pattern: /const canAccessAdmin = roleSet\.has\("admin"\);/
  },
  {
    name: "App shell read-only warning wiring is present",
    pattern: /warning={!canWriteCmdb \? t\("auth\.messages\.readOnly"\) : null}/
  },
  {
    name: "Admin section visibility gate is present",
    pattern: /\{canAccessAdmin && \(/,
  },
  {
    name: "Asset section still exists",
    pattern: /id="section-assets"/
  },
  {
    name: "Relation section still exists",
    pattern: /id="section-relations"/
  }
];

const writeGuardCount = (source.match(/if \(!canWriteCmdb\)/g) ?? []).length;

const requiredLocaleKeys = [
  "auth.messages.readOnly",
  "cmdb.relations.messages.readOnlyHint",
  "cmdb.discovery.messages.readOnlyHint",
  "cmdb.notifications.messages.readOnlyHint",
  "cmdb.assets.filters.searchLabel",
  "cmdb.discovery.messages.loadingJobs",
  "cmdb.notifications.messages.loadingChannels"
];

let hasFailure = false;

for (const check of sourceChecks) {
  if (check.pattern.test(source)) {
    console.log(`PASS: ${check.name}`);
  } else {
    hasFailure = true;
    console.error(`FAIL: ${check.name}`);
  }
}

if (writeGuardCount >= 5) {
  console.log(`PASS: Found ${writeGuardCount} write-guard checks`);
} else {
  hasFailure = true;
  console.error(`FAIL: Expected at least 5 write-guard checks, found ${writeGuardCount}`);
}

for (const keyPath of requiredLocaleKeys) {
  if (hasLocaleKey(locale, keyPath)) {
    console.log(`PASS: Locale key ${keyPath}`);
  } else {
    hasFailure = true;
    console.error(`FAIL: Missing locale key ${keyPath}`);
  }
}

if (hasFailure) {
  console.error("Guardrail checks failed.");
  process.exit(1);
}

console.log("Guardrail checks passed.");

function hasLocaleKey(value, keyPath) {
  const segments = keyPath.split(".");
  let cursor = value;
  for (const segment of segments) {
    if (!cursor || typeof cursor !== "object" || !(segment in cursor)) {
      return false;
    }
    cursor = cursor[segment];
  }
  return true;
}
