#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const appPath = path.resolve(process.cwd(), "src/App.tsx");
const cmdbSectionsPath = path.resolve(process.cwd(), "src/pages/cmdb-sections.tsx");
const overviewAdminSectionsPath = path.resolve(process.cwd(), "src/pages/overview-admin-sections.tsx");
const routingPath = path.resolve(process.cwd(), "src/pages/console-page-routing.ts");
const localePath = path.resolve(process.cwd(), "src/i18n/locales/en-US/common.json");

const appSource = fs.readFileSync(appPath, "utf8");
const cmdbSectionsSource = fs.readFileSync(cmdbSectionsPath, "utf8");
const overviewAdminSectionsSource = fs.readFileSync(overviewAdminSectionsPath, "utf8");
const routingSource = fs.readFileSync(routingPath, "utf8");
const source = [appSource, cmdbSectionsSource, overviewAdminSectionsSource, routingSource].join("\n");
const locale = JSON.parse(fs.readFileSync(localePath, "utf8"));

const sourceChecks = [
  {
    name: "Role write policy (admin/operator) is present",
    source: appSource,
    pattern: /const canWriteCmdb = roleSet\.has\("admin"\) \|\| roleSet\.has\("operator"\);/
  },
  {
    name: "Role admin policy is present",
    source: appSource,
    pattern: /const canAccessAdmin = roleSet\.has\("admin"\);/
  },
  {
    name: "App shell read-only warning wiring is present",
    source: appSource,
    pattern: /warning={!canWriteCmdb \? t\("auth\.messages\.readOnly"\) : null}/
  },
  {
    name: "Admin section visibility gate is present",
    source: overviewAdminSectionsSource,
    pattern: /\{canAccessAdmin && \(/,
  },
  {
    name: "Asset section routing and card anchor exist",
    source: `${routingSource}\n${cmdbSectionsSource}`,
    pattern: /"section-assets"/
  },
  {
    name: "Relation section routing and card anchor exist",
    source: `${routingSource}\n${cmdbSectionsSource}`,
    pattern: /"section-relations"/
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
  if (check.pattern.test(check.source)) {
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
