# frozen_string_literal: true

require "shellwords"

module GitLogRatatui
  module Git
    Commit = Data.define(:hash, :author, :date, :subject)

    def self.commits(path = ".", branch: nil)
      scope = branch ? Shellwords.escape(branch) : "--all"
      output = `git -C #{Shellwords.escape(path)} log #{scope} --oneline --decorate --format=%H%x00%an%x00%ad%x00%s --date=short 2>/dev/null`
      return [] unless $?.success?

      output.split("\n").map do |line|
        parts = line.split("\0")
        next unless parts.size == 4

        Commit.new(hash: parts[0], author: parts[1], date: parts[2], subject: parts[3])
      end.compact
    end

    def self.branches(path = ".")
      output = `git -C #{Shellwords.escape(path)} branch --format='%(refname:short)' 2>/dev/null`
      return [] unless $?.success?

      names = output.split("\n").map(&:strip).reject(&:empty?)

      primary = []
      rest = []

      names.each do |name|
        if name == "main" || name == "master"
          primary << name
        else
          rest << name
        end
      end

      primary.sort + rest.sort
    end
  end
end
