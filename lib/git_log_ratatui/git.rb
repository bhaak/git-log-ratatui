# frozen_string_literal: true

require "shellwords"

module GitLogRatatui
  module Git
    Commit = Data.define(:hash, :author, :date, :subject)

    def self.commits(path = ".")
      output = `git -C #{Shellwords.escape(path)} log --all --oneline --decorate --format=%H%x00%an%x00%ad%x00%s --date=short 2>/dev/null`
      return [] unless $?.success?

      output.split("\n").map do |line|
        parts = line.split("\0")
        next unless parts.size == 4

        Commit.new(hash: parts[0], author: parts[1], date: parts[2], subject: parts[3])
      end.compact
    end
  end
end
