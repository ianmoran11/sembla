#!/usr/bin/env ruby
# frozen_string_literal: true

require "yaml"

workflows = Dir.glob(File.expand_path("../.github/workflows/*.{yml,yaml}", __dir__)).sort
abort "error: no GitHub Actions workflows found" if workflows.empty?

workflows.each do |path|
  document = YAML.safe_load(File.read(path, encoding: "UTF-8"), permitted_classes: [], aliases: false)
  raise "#{path}: workflow root must be a mapping" unless document.is_a?(Hash)

  # Psych implements YAML 1.1 and therefore parses the unquoted GitHub key
  # `on` as boolean true. Accept either representation while checking shape.
  triggers = document["on"] || document[true]
  raise "#{path}: workflow must declare event triggers" unless triggers.is_a?(Hash)
  raise "#{path}: workflow must declare jobs" unless document["jobs"].is_a?(Hash)

  basename = File.basename(path)
  trigger_names = triggers.keys.map(&:to_s).sort
  case basename
  when "ci.yml"
    required = %w[pull_request push]
    missing = required - trigger_names
    raise "#{path}: missing triggers: #{missing.join(', ')}" unless missing.empty?
  when "gpu-differential.yml"
    unless trigger_names == ["workflow_dispatch"]
      raise "#{path}: GPU stub must be workflow_dispatch-only (found #{trigger_names.join(', ')})"
    end
  end

  puts "parsed #{path}"
end
