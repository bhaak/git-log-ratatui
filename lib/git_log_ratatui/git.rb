# frozen_string_literal: true

require "shellwords"

module GitLogRatatui
  module Git
    Commit = Data.define(:hash, :author, :date, :subject, :graph)

    GRAPH_LINE_RE = /\A([ *|\/\\_]*?) (\h{40})\0/

    def self.commits(path = ".", branch: nil)
      scope = branch ? Shellwords.escape(branch) : "--all"
      fmt = "--format=%H%x00%an%x00%ad%x00%s --date=format:'%Y-%m-%d %H:%M'"
      output = `git -C #{Shellwords.escape(path)} log #{scope} --graph #{fmt} 2>/dev/null`
      return [] unless $?.success?

      output.split("\n").filter_map do |line|
        md = line.match(GRAPH_LINE_RE)
        next unless md

        parts = line[md.end(0)..].split("\0")
        next unless parts.size == 3

        Commit.new(graph: unicode_graph(md[1]), hash: md[2], author: parts[0], date: parts[1], subject: parts[2])
      end
    end

    def self.unicode_graph(graph)
      graph
        .tr("|", "│")
        .tr("/", "╱")
        .tr("\\", "╲")
        .tr("_", "─")
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

    def self.diff(path, commit_hash)
      output = `git -C #{Shellwords.escape(path)} show #{Shellwords.escape(commit_hash)} --format='' 2>/dev/null`
      return [] unless $?.success?

      output.split("\n")
    end

    CommitInfo = Data.define(:subject, :hash, :parents, :author_name, :author_email, :author_date, :committer_name, :committer_email, :committer_date)

    def self.commit_info(path, commit_hash)
      fmt = "%s%x00%H%x00%P%x00%an%x00%ae%x00%ai%x00%cn%x00%ce%x00%ci"
      output = `git -C #{Shellwords.escape(path)} show #{Shellwords.escape(commit_hash)} --no-patch --format='#{fmt}' 2>/dev/null`
      return nil unless $?.success?

      parts = output.chomp.split("\0")
      return nil unless parts.size == 9

      CommitInfo.new(
        subject: parts[0],
        hash: parts[1],
        parents: parts[2],
        author_name: parts[3],
        author_email: parts[4],
        author_date: parts[5],
        committer_name: parts[6],
        committer_email: parts[7],
        committer_date: parts[8]
      )
    end
  end
end
