#!/usr/bin/env ruby

begin
  require 'table_print'
rescue LoadError => e
    puts "Error: Ruby gem `table_print` not installed (sudo gem install table_print)"
    Process.exit(1)
end

# This is a super hacky patch that removes the "nested hash" support.
# Why this was added is incredibly assuming, so I'm removing it here
# since it thinks version name columns are nested hashes.
# Ex: "v0.1.0-alpha.5-17-geb59a3f"
module TablePrint
  class Fingerprinter
    def display_method_to_nested_hash(display_method)
      hash = {}

      return {display_method => {}} if display_method.is_a? Proc
      [display_method].inject(hash) do |hash_level, method|
        hash_level[method] ||= {}
      end
      hash
    end
  end
end

tp.set :capitalize_headers, false # don't capitalize column headers
tp.set :multibyte, true