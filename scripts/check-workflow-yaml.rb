#!/usr/bin/env ruby
# frozen_string_literal: true

require "yaml"

SHA_PIN = /\A[^@\s]+@[0-9a-f]{40}\z/
RELEASE_COMMENT = /\Av\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?\z/
# Repository-local actions are content-addressed by the checkout itself and do
# not have an upstream release SHA. Only explicit relative paths are exempt.
LOCAL_ACTION_PREFIX = "./"


def load_yaml(path)
  YAML.safe_load(File.read(path, encoding: "UTF-8"), permitted_classes: [], aliases: false)
end


def collect_uses(node, location = [], references = [])
  case node
  when Hash
    node.each do |key, value|
      key_location = location + [key.to_s]
      if key.to_s == "uses"
        raise "#{key_location.join('.')}: uses value must be a string" unless value.is_a?(String)

        references << [value, key_location.join(".")]
      else
        collect_uses(value, key_location, references)
      end
    end
  when Array
    node.each_with_index { |value, index| collect_uses(value, location + [index.to_s], references) }
  end

  references
end


def source_uses_entries(path)
  entries = []
  File.readlines(path, encoding: "UTF-8").each_with_index do |line, index|
    match = line.match(/\A\s*(?:-\s*)?uses:\s*(?<value>'[^']*'|"[^"]*"|[^#\s]+)\s*(?:#\s*(?<comment>.*?)\s*)?\z/)
    next unless match

    value = match[:value]
    value = value[1...-1] if (value.start_with?("'") && value.end_with?("'")) ||
                              (value.start_with?("\"") && value.end_with?("\""))
    entries << [value, match[:comment], index + 1]
  end
  entries
end


def validate_action_references(path, document)
  parsed = collect_uses(document)
  source = source_uses_entries(path)
  parsed_values = parsed.map(&:first)
  source_values = source.map(&:first)
  counts = lambda do |values|
    values.each_with_object(Hash.new(0)) { |value, result| result[value] += 1 }
  end
  unless counts.call(parsed_values) == counts.call(source_values)
    raise "#{path}: source uses entries do not match parsed YAML uses values"
  end

  parsed.each do |value, location|
    next if value.start_with?(LOCAL_ACTION_PREFIX)

    raise "#{path}: #{location} must pin #{value.inspect} to a full 40-character commit SHA" unless SHA_PIN.match?(value)
  end

  source.each do |value, comment, line|
    next if value.start_with?(LOCAL_ACTION_PREFIX)

    unless RELEASE_COMMENT.match?(comment.to_s)
      raise "#{path}:#{line}: immutable action pin #{value.inspect} needs an adjacent release comment such as '# v4.2.2'"
    end
  end
end


def validate_npe_filter(path, document)
  changes = document.fetch("jobs").fetch("changes")
  timeout = changes["timeout-minutes"]
  raise "#{path}: changes job needs a short timeout-minutes value" unless timeout.is_a?(Integer) && timeout.positive? && timeout <= 10

  filter_step = changes.fetch("steps").find { |step| step.is_a?(Hash) && step["id"] == "filter" }
  raise "#{path}: changes job must define the path-filter step" unless filter_step

  filters = YAML.safe_load(filter_step.fetch("with").fetch("filters"), permitted_classes: [], aliases: false)
  npe_paths = filters.fetch("npe")
  required_paths = [
    "calibration/**",
    "calibration/npe/requirements.txt",
    "calibration/npe/requirements-ci.lock",
    "docs/prds-npe-path/**",
    "scripts/check-npe-smoke.sh",
    ".github/workflows/ci.yml"
  ]
  missing = required_paths - npe_paths
  raise "#{path}: NPE filter missing self-test paths: #{missing.join(', ')}" unless missing.empty?
end


def validate_permissions(path, document)
  expected = { "contents" => "read" }
  raise "#{path}: top-level permissions must remain read-only" unless document["permissions"] == expected
end

workflows = Dir.glob(File.expand_path("../.github/workflows/*.{yml,yaml}", __dir__)).sort
abort "error: no GitHub Actions workflows found" if workflows.empty?

workflows.each do |path|
  document = load_yaml(path)
  raise "#{path}: workflow root must be a mapping" unless document.is_a?(Hash)

  # Psych implements YAML 1.1 and therefore parses the unquoted GitHub key
  # `on` as boolean true. Accept either representation while checking shape.
  triggers = document["on"] || document[true]
  raise "#{path}: workflow must declare event triggers" unless triggers.is_a?(Hash)
  raise "#{path}: workflow must declare jobs" unless document["jobs"].is_a?(Hash)

  validate_action_references(path, document)
  validate_permissions(path, document)

  basename = File.basename(path)
  trigger_names = triggers.keys.map(&:to_s).sort
  case basename
  when "ci.yml"
    required = %w[pull_request push]
    missing = required - trigger_names
    raise "#{path}: missing triggers: #{missing.join(', ')}" unless missing.empty?
    validate_npe_filter(path, document)
  when "gpu-differential.yml"
    unless trigger_names == ["workflow_dispatch"]
      raise "#{path}: GPU stub must be workflow_dispatch-only (found #{trigger_names.join(', ')})"
    end
    runbook = document.fetch("jobs").fetch("runbook")
    timeout = runbook["timeout-minutes"]
    raise "#{path}: GPU stub needs a short timeout-minutes value" unless timeout.is_a?(Integer) && timeout.positive? && timeout <= 10
  end

  puts "parsed #{path}"
end

dependabot_path = File.expand_path("../.github/dependabot.yml", __dir__)
dependabot = load_yaml(dependabot_path)
raise "#{dependabot_path}: version must be 2" unless dependabot.is_a?(Hash) && dependabot["version"] == 2
updates = dependabot["updates"]
raise "#{dependabot_path}: updates must contain exactly one entry" unless updates.is_a?(Array) && updates.length == 1
update = updates.first
raise "#{dependabot_path}: only github-actions updates are allowed" unless update["package-ecosystem"] == "github-actions"
raise "#{dependabot_path}: github-actions directory must be /" unless update["directory"] == "/"
raise "#{dependabot_path}: github-actions cadence must be weekly" unless update.dig("schedule", "interval") == "weekly"
groups = update["groups"]
unless groups.is_a?(Hash) && groups.values.any? { |group| group.is_a?(Hash) && group["patterns"] == ["*"] }
  raise "#{dependabot_path}: github-actions updates must be grouped with a '*' pattern"
end
puts "parsed #{dependabot_path}"
