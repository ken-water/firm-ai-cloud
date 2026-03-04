#!/usr/bin/env bash
set -Eeuo pipefail

API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
AUTH_USER="${AUTH_USER:-admin}"
DEMO_TAG="${DEMO_TAG:-demo-$(date +%Y%m%d%H%M%S)}"
RUN_DIR="${RUN_DIR:-.run/demo}"
MANIFEST_FILE="${RUN_DIR}/${DEMO_TAG}-manifest.json"
SNAPSHOT_DIR="${RUN_DIR}/${DEMO_TAG}-snapshots"

log() {
  echo "[demo-seed] $*"
}

require_tool() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "ERROR: required command not found: $1" >&2
    exit 1
  fi
}

api_curl() {
  curl -fsS -H "x-auth-user: ${AUTH_USER}" "$@"
}

api_get() {
  api_curl "$1"
}

api_post_json() {
  local url="$1"
  local body="$2"
  api_curl -X POST "$url" -H "Content-Type: application/json" -d "$body"
}

api_put_json() {
  local url="$1"
  local body="$2"
  api_curl -X PUT "$url" -H "Content-Type: application/json" -d "$body"
}

snapshot_get() {
  local url="$1"
  local output_file="$2"
  if ! api_get "$url" >"$output_file"; then
    log "Snapshot request failed, wrote fallback payload: ${url}"
    jq -nc --arg url "$url" --arg error "request_failed" '{url: $url, error: $error}' >"$output_file"
  fi
}

extract_id() {
  jq -er '.id | select(type == "number")'
}

require_numeric_id() {
  local value="$1"
  local label="$2"
  if [[ ! "$value" =~ ^[0-9]+$ ]]; then
    echo "ERROR: invalid id for ${label}: ${value}" >&2
    exit 1
  fi
}

create_monitoring_source() {
  local name="$1"
  local endpoint="$2"
  local auth_type="$3"
  local username="$4"
  local secret_ref="$5"
  local site="$6"
  local department="$7"
  local enabled="$8"

  local payload
  payload="$(jq -nc \
    --arg name "$name" \
    --arg endpoint "$endpoint" \
    --arg auth_type "$auth_type" \
    --arg username "$username" \
    --arg secret_ref "$secret_ref" \
    --arg site "$site" \
    --arg department "$department" \
    --argjson enabled "$enabled" \
    '{
      name: $name,
      source_type: "zabbix",
      endpoint: $endpoint,
      auth_type: $auth_type,
      username: (if $username == "" then null else $username end),
      secret_ref: $secret_ref,
      site: (if $site == "" then null else $site end),
      department: (if $department == "" then null else $department end),
      is_enabled: $enabled
    }')"

  api_post_json "${API_BASE_URL}/api/v1/monitoring/sources" "$payload"
}

probe_monitoring_source() {
  local source_id="$1"
  api_curl -X POST "${API_BASE_URL}/api/v1/monitoring/sources/${source_id}/probe"
}

create_asset() {
  local asset_class="$1"
  local name="$2"
  local hostname="$3"
  local ip="$4"
  local site="$5"
  local department="$6"
  local owner="$7"
  local qr_code="$8"
  local barcode="$9"
  local custom_fields="${10}"

  local payload
  payload="$(jq -nc \
    --arg asset_class "$asset_class" \
    --arg name "$name" \
    --arg hostname "$hostname" \
    --arg ip "$ip" \
    --arg site "$site" \
    --arg department "$department" \
    --arg owner "$owner" \
    --arg qr_code "$qr_code" \
    --arg barcode "$barcode" \
    --argjson custom_fields "$custom_fields" \
    '{
      asset_class: $asset_class,
      name: $name,
      hostname: (if $hostname == "" then null else $hostname end),
      ip: (if $ip == "" then null else $ip end),
      status: "idle",
      site: (if $site == "" then null else $site end),
      department: (if $department == "" then null else $department end),
      owner: (if $owner == "" then null else $owner end),
      qr_code: (if $qr_code == "" then null else $qr_code end),
      barcode: (if $barcode == "" then null else $barcode end),
      custom_fields: $custom_fields
    }')"

  api_post_json "${API_BASE_URL}/api/v1/cmdb/assets" "$payload"
}

create_relation() {
  local src_asset_id="$1"
  local dst_asset_id="$2"
  local relation_type="$3"
  local source="$4"

  local payload
  payload="$(jq -nc \
    --argjson src_asset_id "$src_asset_id" \
    --argjson dst_asset_id "$dst_asset_id" \
    --arg relation_type "$relation_type" \
    --arg source "$source" \
    '{
      src_asset_id: $src_asset_id,
      dst_asset_id: $dst_asset_id,
      relation_type: $relation_type,
      source: $source
    }')"

  api_post_json "${API_BASE_URL}/api/v1/cmdb/relations" "$payload"
}

upsert_bindings() {
  local asset_id="$1"
  local body="$2"
  api_put_json "${API_BASE_URL}/api/v1/cmdb/assets/${asset_id}/bindings" "$body"
}

transition_operational() {
  local asset_id="$1"
  api_post_json "${API_BASE_URL}/api/v1/cmdb/assets/${asset_id}/lifecycle" '{"status":"operational"}'
}

trigger_monitoring_sync() {
  local asset_id="$1"
  local reason="$2"
  local payload
  payload="$(jq -nc --arg reason "$reason" '{reason: $reason}')"
  api_post_json "${API_BASE_URL}/api/v1/cmdb/assets/${asset_id}/monitoring-sync" "$payload"
}

create_discovery_job() {
  local name="$1"
  local scope_json="$2"
  local payload
  payload="$(jq -nc \
    --arg name "$name" \
    --argjson scope "$scope_json" \
    '{
      name: $name,
      source_type: "zabbix_hosts",
      scope: $scope
    }')"

  api_post_json "${API_BASE_URL}/api/v1/cmdb/discovery/jobs" "$payload"
}

create_notification_channel() {
  local name="$1"
  local target="$2"
  local payload
  payload="$(jq -nc \
    --arg name "$name" \
    --arg target "$target" \
    '{
      name: $name,
      channel_type: "webhook",
      target: $target,
      config: {
        max_attempts: 2,
        base_delay_ms: 100
      }
    }')"
  api_post_json "${API_BASE_URL}/api/v1/cmdb/discovery/notification-channels" "$payload"
}

create_notification_template() {
  local event_type="$1"
  local title_template="$2"
  local body_template="$3"
  local payload
  payload="$(jq -nc \
    --arg event_type "$event_type" \
    --arg title_template "$title_template" \
    --arg body_template "$body_template" \
    '{
      event_type: $event_type,
      title_template: $title_template,
      body_template: $body_template
    }')"
  api_post_json "${API_BASE_URL}/api/v1/cmdb/discovery/notification-templates" "$payload"
}

create_notification_subscription() {
  local channel_id="$1"
  local event_type="$2"
  local site="$3"
  local department="$4"
  local payload
  payload="$(jq -nc \
    --argjson channel_id "$channel_id" \
    --arg event_type "$event_type" \
    --arg site "$site" \
    --arg department "$department" \
    '{
      channel_id: $channel_id,
      event_type: $event_type,
      site: (if $site == "" then null else $site end),
      department: (if $department == "" then null else $department end)
    }')"
  api_post_json "${API_BASE_URL}/api/v1/cmdb/discovery/notification-subscriptions" "$payload"
}

main() {
  require_tool curl
  require_tool jq

  mkdir -p "$RUN_DIR" "$SNAPSHOT_DIR"

  log "Health check: ${API_BASE_URL}/health"
  curl -fsS "${API_BASE_URL}/health" >/dev/null

  log "Loading enabled required custom field defaults"
  local field_defs
  field_defs="$(api_get "${API_BASE_URL}/api/v1/cmdb/field-definitions")"
  local required_custom_fields
  required_custom_fields="$(echo "$field_defs" | jq -c '
    def text_default($max):
      if ($max | type) == "number" then
        if $max <= 0 then
          "x"
        else
          ("demo"[0:$max])
        end
      else
        "demo"
      end;

    reduce .[] as $d ({};
      if ($d.required == true and $d.is_enabled == true) then
        . + {
          ($d.field_key):
            (if $d.field_type == "text" then text_default($d.max_length)
             elif $d.field_type == "integer" then 1
             elif $d.field_type == "float" then 1.5
             elif $d.field_type == "boolean" then true
             elif $d.field_type == "enum" then
               (if (($d.options // []) | length) > 0 then ($d.options[0]) else "demo" end)
             elif $d.field_type == "date" then "2026-03-03"
             elif $d.field_type == "datetime" then "2026-03-03T00:00:00Z"
             else "demo" end)
        }
      else . end
    )')"

  log "Creating monitoring sources"
  local src_primary src_backup src_lab
  src_primary="$(create_monitoring_source \
    "ms-primary-${DEMO_TAG}" \
    "http://127.0.0.1:8082/api_jsonrpc.php" \
    "basic" \
    "Admin" \
    "plain:zabbix" \
    "dc-a" \
    "platform" \
    "true")"
  src_backup="$(create_monitoring_source \
    "ms-backup-${DEMO_TAG}" \
    "http://127.0.0.1:65535/unreachable" \
    "token" \
    "" \
    "plain:dummy-token" \
    "dc-b" \
    "network" \
    "true")"
  src_lab="$(create_monitoring_source \
    "ms-lab-${DEMO_TAG}" \
    "http://127.0.0.1:8082/api_jsonrpc.php" \
    "token" \
    "" \
    "plain:lab-token" \
    "lab" \
    "qa" \
    "false")"

  local src_primary_id src_backup_id src_lab_id
  src_primary_id="$(echo "$src_primary" | extract_id)"
  src_backup_id="$(echo "$src_backup" | extract_id)"
  src_lab_id="$(echo "$src_lab" | extract_id)"
  require_numeric_id "$src_primary_id" "monitoring source primary"
  require_numeric_id "$src_backup_id" "monitoring source backup"
  require_numeric_id "$src_lab_id" "monitoring source lab"

  log "Probing monitoring sources"
  probe_monitoring_source "$src_primary_id" >"${SNAPSHOT_DIR}/probe-primary.json"
  probe_monitoring_source "$src_backup_id" >"${SNAPSHOT_DIR}/probe-backup.json" || true

  log "Creating assets for hierarchy, network, service and business demo"
  local a_phy a_vm1 a_vm2 a_ct1 a_db1 a_app1 a_sw1 a_fw1 a_bs_orders a_bs_payment a_team_platform a_team_dba
  a_phy="$(create_asset "physical_host" "phy-${DEMO_TAG}" "phy-${DEMO_TAG}.local" "10.90.0.10" "dc-a" "platform" "infra" "QR-PHY-${DEMO_TAG}" "BC-PHY-${DEMO_TAG}" "$required_custom_fields")"
  a_vm1="$(create_asset "virtual_machine" "vm-app-${DEMO_TAG}" "vm-app-${DEMO_TAG}.local" "10.90.0.11" "dc-a" "platform" "appops" "QR-VM1-${DEMO_TAG}" "BC-VM1-${DEMO_TAG}" "$required_custom_fields")"
  a_vm2="$(create_asset "virtual_machine" "vm-db-${DEMO_TAG}" "vm-db-${DEMO_TAG}.local" "10.90.0.12" "dc-a" "dba" "dba" "QR-VM2-${DEMO_TAG}" "BC-VM2-${DEMO_TAG}" "$required_custom_fields")"
  a_ct1="$(create_asset "container" "ct-orders-${DEMO_TAG}" "ct-orders-${DEMO_TAG}.local" "10.90.0.21" "dc-a" "platform" "appops" "" "" "$required_custom_fields")"
  a_db1="$(create_asset "database" "db-orders-${DEMO_TAG}" "db-orders-${DEMO_TAG}.local" "10.90.0.31" "dc-a" "dba" "dba" "" "" "$required_custom_fields")"
  a_app1="$(create_asset "server" "app-orders-${DEMO_TAG}" "app-orders-${DEMO_TAG}.local" "10.90.0.41" "dc-a" "platform" "appops" "" "" "$required_custom_fields")"
  a_sw1="$(create_asset "network_device" "sw-core-${DEMO_TAG}" "sw-core-${DEMO_TAG}.local" "10.90.1.11" "dc-a" "network" "noc" "" "" "$required_custom_fields")"
  a_fw1="$(create_asset "network_device" "fw-edge-${DEMO_TAG}" "fw-edge-${DEMO_TAG}.local" "10.90.1.12" "dc-b" "network" "noc" "" "" "$required_custom_fields")"
  a_bs_orders="$(create_asset "business_service" "svc-orders-${DEMO_TAG}" "" "" "dc-a" "biz" "biz-owner" "" "" "$required_custom_fields")"
  a_bs_payment="$(create_asset "business_service" "svc-payment-${DEMO_TAG}" "" "" "dc-b" "biz" "biz-owner" "" "" "$required_custom_fields")"
  a_team_platform="$(create_asset "team" "team-platform-${DEMO_TAG}" "" "" "dc-a" "platform" "platform-lead" "" "" "$required_custom_fields")"
  a_team_dba="$(create_asset "team" "team-dba-${DEMO_TAG}" "" "" "dc-a" "dba" "dba-lead" "" "" "$required_custom_fields")"

  local phy_id vm1_id vm2_id ct1_id db1_id app1_id sw1_id fw1_id bs_orders_id bs_payment_id team_platform_id team_dba_id
  phy_id="$(echo "$a_phy" | extract_id)"
  vm1_id="$(echo "$a_vm1" | extract_id)"
  vm2_id="$(echo "$a_vm2" | extract_id)"
  ct1_id="$(echo "$a_ct1" | extract_id)"
  db1_id="$(echo "$a_db1" | extract_id)"
  app1_id="$(echo "$a_app1" | extract_id)"
  sw1_id="$(echo "$a_sw1" | extract_id)"
  fw1_id="$(echo "$a_fw1" | extract_id)"
  bs_orders_id="$(echo "$a_bs_orders" | extract_id)"
  bs_payment_id="$(echo "$a_bs_payment" | extract_id)"
  team_platform_id="$(echo "$a_team_platform" | extract_id)"
  team_dba_id="$(echo "$a_team_dba" | extract_id)"
  require_numeric_id "$phy_id" "asset physical host"
  require_numeric_id "$vm1_id" "asset vm1"
  require_numeric_id "$vm2_id" "asset vm2"
  require_numeric_id "$ct1_id" "asset container"
  require_numeric_id "$db1_id" "asset database"
  require_numeric_id "$app1_id" "asset app"
  require_numeric_id "$sw1_id" "asset switch"
  require_numeric_id "$fw1_id" "asset firewall"
  require_numeric_id "$bs_orders_id" "asset business service orders"
  require_numeric_id "$bs_payment_id" "asset business service payment"
  require_numeric_id "$team_platform_id" "asset team platform"
  require_numeric_id "$team_dba_id" "asset team dba"

  log "Creating asset relations"
  create_relation "$phy_id" "$vm1_id" "contains" "manual" >/dev/null
  create_relation "$phy_id" "$vm2_id" "contains" "manual" >/dev/null
  create_relation "$vm1_id" "$ct1_id" "contains" "manual" >/dev/null
  create_relation "$app1_id" "$db1_id" "depends_on" "manual" >/dev/null
  create_relation "$vm1_id" "$db1_id" "depends_on" "manual" >/dev/null
  create_relation "$vm1_id" "$bs_orders_id" "runs_service" "manual" >/dev/null
  create_relation "$db1_id" "$bs_orders_id" "runs_service" "manual" >/dev/null
  create_relation "$db1_id" "$bs_payment_id" "runs_service" "manual" >/dev/null
  create_relation "$vm1_id" "$team_platform_id" "owned_by" "manual" >/dev/null
  create_relation "$db1_id" "$team_dba_id" "owned_by" "manual" >/dev/null
  create_relation "$sw1_id" "$fw1_id" "depends_on" "manual" >/dev/null

  log "Upserting bindings and lifecycle transitions"
  upsert_bindings "$vm1_id" "$(jq -nc --arg svc "orders-${DEMO_TAG}" --arg owner "platform-${DEMO_TAG}" '{
    departments: ["platform", "sre"],
    business_services: [$svc],
    owners: [
      {owner_type: "team", owner_ref: $owner},
      {owner_type: "user", owner_ref: "alice"}
    ]
  }')" >"${SNAPSHOT_DIR}/bindings-vm1.json"
  upsert_bindings "$db1_id" "$(jq -nc --arg svc "orders-${DEMO_TAG}" --arg svc2 "payment-${DEMO_TAG}" --arg owner "dba-${DEMO_TAG}" '{
    departments: ["dba"],
    business_services: [$svc, $svc2],
    owners: [
      {owner_type: "team", owner_ref: $owner},
      {owner_type: "group", owner_ref: "database-admins"}
    ]
  }')" >"${SNAPSHOT_DIR}/bindings-db1.json"
  transition_operational "$vm1_id" >"${SNAPSHOT_DIR}/lifecycle-vm1.json"
  transition_operational "$db1_id" >"${SNAPSHOT_DIR}/lifecycle-db1.json"

  log "Enqueue monitoring sync jobs"
  trigger_monitoring_sync "$vm1_id" "demo baseline seed ${DEMO_TAG}" >"${SNAPSHOT_DIR}/sync-vm1.json"
  trigger_monitoring_sync "$db1_id" "demo baseline seed ${DEMO_TAG}" >"${SNAPSHOT_DIR}/sync-db1.json"
  trigger_monitoring_sync "$sw1_id" "demo baseline seed ${DEMO_TAG}" >"${SNAPSHOT_DIR}/sync-sw1.json"

  log "Creating discovery jobs and running discovery"
  local discovery_scope_a discovery_scope_b job_a job_b job_a_id job_b_id
  discovery_scope_a="$(jq -nc --arg tag "$DEMO_TAG" '{
    offboarded_threshold: 2,
    mock_hosts: [
      {name: ("new-app-" + $tag), hostname: ("new-app-" + $tag + ".local"), ip: "10.90.2.21", asset_class: "server"},
      {name: ("new-sw-" + $tag), hostname: ("new-sw-" + $tag + ".local"), ip: "10.90.2.22", asset_class: "network_device"}
    ]
  }')"
  discovery_scope_b="$(jq -nc --arg tag "$DEMO_TAG" '{
    offboarded_threshold: 2,
    mock_hosts: [
      {name: ("merge-db-" + $tag), hostname: ("merge-db-" + $tag + ".local"), ip: "10.90.2.31", asset_class: "database"}
    ]
  }')"

  job_a="$(create_discovery_job "discovery-a-${DEMO_TAG}" "$discovery_scope_a")"
  job_b="$(create_discovery_job "discovery-b-${DEMO_TAG}" "$discovery_scope_b")"
  job_a_id="$(echo "$job_a" | extract_id)"
  job_b_id="$(echo "$job_b" | extract_id)"
  require_numeric_id "$job_a_id" "discovery job A"
  require_numeric_id "$job_b_id" "discovery job B"
  api_curl -X POST "${API_BASE_URL}/api/v1/cmdb/discovery/jobs/${job_a_id}/run" >"${SNAPSHOT_DIR}/discovery-run-a.json"
  api_curl -X POST "${API_BASE_URL}/api/v1/cmdb/discovery/jobs/${job_b_id}/run" >"${SNAPSHOT_DIR}/discovery-run-b.json"

  log "Creating notification channel/template/subscription and triggering delivery"
  local channel channel_id template_event
  channel="$(create_notification_channel "webhook-${DEMO_TAG}" "http://127.0.0.1:65535/demo-${DEMO_TAG}")"
  channel_id="$(echo "$channel" | extract_id)"
  require_numeric_id "$channel_id" "notification channel"
  template_event="asset.new_detected"
  create_notification_template "$template_event" "[demo:${DEMO_TAG}] New Asset {{event_type}}" "payload={{payload}}" >"${SNAPSHOT_DIR}/notification-template.json"
  create_notification_subscription "$channel_id" "$template_event" "" "" >"${SNAPSHOT_DIR}/notification-subscription.json"
  api_curl -X POST "${API_BASE_URL}/api/v1/cmdb/discovery/jobs/${job_a_id}/run" >"${SNAPSHOT_DIR}/discovery-run-a-notify.json"

  log "Collecting demo snapshots"
  snapshot_get "${API_BASE_URL}/api/v1/monitoring/overview?site=dc-a" "${SNAPSHOT_DIR}/monitoring-overview-dca.json"
  snapshot_get "${API_BASE_URL}/api/v1/monitoring/layers/hardware?site=dc-a&limit=50" "${SNAPSHOT_DIR}/monitoring-layer-hardware-dca.json"
  snapshot_get "${API_BASE_URL}/api/v1/cmdb/assets/${phy_id}/impact?direction=downstream&depth=4" "${SNAPSHOT_DIR}/impact-physical-root.json"
  snapshot_get "${API_BASE_URL}/api/v1/cmdb/assets/${vm1_id}/monitoring-binding" "${SNAPSHOT_DIR}/monitoring-binding-vm1.json"
  snapshot_get "${API_BASE_URL}/api/v1/cmdb/assets/${db1_id}/monitoring-binding" "${SNAPSHOT_DIR}/monitoring-binding-db1.json"
  snapshot_get "${API_BASE_URL}/api/v1/cmdb/assets?limit=200&q=${DEMO_TAG}" "${SNAPSHOT_DIR}/assets-search-demo-tag.json"
  snapshot_get "${API_BASE_URL}/api/v1/cmdb/discovery/candidates?review_status=pending&limit=50" "${SNAPSHOT_DIR}/discovery-candidates-pending.json"
  snapshot_get "${API_BASE_URL}/api/v1/cmdb/monitoring-sync/jobs?limit=100" "${SNAPSHOT_DIR}/monitoring-sync-jobs.json"
  snapshot_get "${API_BASE_URL}/api/v1/cmdb/discovery/notification-deliveries?limit=100" "${SNAPSHOT_DIR}/notification-deliveries.json"

  log "Writing manifest: ${MANIFEST_FILE}"
  jq -nc \
    --arg demo_tag "$DEMO_TAG" \
    --arg created_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --arg api_base_url "$API_BASE_URL" \
    --arg auth_user "$AUTH_USER" \
    --arg snapshot_dir "$SNAPSHOT_DIR" \
    --argjson monitoring_source_ids "$(jq -nc --argjson a "$src_primary_id" --argjson b "$src_backup_id" --argjson c "$src_lab_id" '[ $a, $b, $c ]')" \
    --argjson asset_ids "$(jq -nc \
      --argjson phy "$phy_id" \
      --argjson vm1 "$vm1_id" \
      --argjson vm2 "$vm2_id" \
      --argjson ct1 "$ct1_id" \
      --argjson db1 "$db1_id" \
      --argjson app1 "$app1_id" \
      --argjson sw1 "$sw1_id" \
      --argjson fw1 "$fw1_id" \
      --argjson bso "$bs_orders_id" \
      --argjson bsp "$bs_payment_id" \
      --argjson tpf "$team_platform_id" \
      --argjson tdb "$team_dba_id" \
      '[ $phy, $vm1, $vm2, $ct1, $db1, $app1, $sw1, $fw1, $bso, $bsp, $tpf, $tdb ]')" \
    --argjson discovery_job_ids "$(jq -nc --argjson a "$job_a_id" --argjson b "$job_b_id" '[ $a, $b ]')" \
    --argjson notification_channel_id "$channel_id" \
    '{
      demo_tag: $demo_tag,
      created_at: $created_at,
      api_base_url: $api_base_url,
      auth_user: $auth_user,
      snapshot_dir: $snapshot_dir,
      monitoring_source_ids: $monitoring_source_ids,
      asset_ids: $asset_ids,
      discovery_job_ids: $discovery_job_ids,
      notification_channel_id: $notification_channel_id
    }' >"$MANIFEST_FILE"

  log "Demo data seed completed"
  log "Demo tag: ${DEMO_TAG}"
  log "Manifest: ${MANIFEST_FILE}"
  log "Snapshots: ${SNAPSHOT_DIR}"
  log "Next:"
  log "  bash scripts/demo-health-check.sh"
  log "  bash scripts/demo-cleanup-data.sh --tag ${DEMO_TAG}"
}

main "$@"
