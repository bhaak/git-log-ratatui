# frozen_string_literal: true

module GitLogRatatui
  module Git
    Commit = Data.define(:hash, :author, :date, :subject)

    COMMAND = %w[git log --all --oneline --decorate --format=%H%x00%an%x00%ad%x00%s --date=short].freeze

    def self.commits
      output = `#{COMMAND.join(' ')} 2>/dev/null`
      return [] unless $?.success?

      output.split("\n").map do |line|
        parts = line.split("\0")
        next unless parts.size == 4

        Commit.new(hash: parts[0], author: parts[1], date: parts[2], subject: parts[3])
      end.compact
    end
  end
end
