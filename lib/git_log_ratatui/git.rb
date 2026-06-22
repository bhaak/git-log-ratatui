# frozen_string_literal: true

require "shellwords"

module GitLogRatatui
  module Git
    Commit = Data.define(:hash, :author, :date, :subject, :graph, :merge, :graph_only, :decorations, :deco_line)

    GRAPH_LINE_RE = /\A([ *|\/\\_]*?) (\h{40})\0/
    GRAPH_ONLY_RE = /\A([|\/\\_ ]+)\z/

    def self.commits(path = ".", branch: nil)
      scope = branch ? Shellwords.escape(branch) : "--all"
      fmt = "--format=%H%x00%an%x00%ad%x00%s%x00%d%x00%P --date=format:'%Y-%m-%d %H:%M'"
      output = `git -C #{Shellwords.escape(path)} log #{scope} --graph #{fmt} 2>/dev/null`
      return [] unless $?.success?

      prev_decorations = ""
      output.split("\n").filter_map do |line|
        md = line.match(GRAPH_LINE_RE)
        if md
          parts = line[md.end(0)..].split("\0", -1)
          next if parts.size < 3

          parents_str = parts.pop || ""
          decorations = parse_decorations(parts.pop || "")
          subject = parts.pop || ""
          date = parts.pop || ""
          author = parts.pop || ""
          parents = parents_str.split

          prev_decorations = decorations
          Commit.new(
            graph: unicode_graph(md[1]),
            hash: md[2],
            author: author,
            date: date,
            subject: subject,
            merge: parents.size > 1,
            graph_only: false,
            decorations: decorations,
            deco_line: false
          )
        elsif (gm = line.match(GRAPH_ONLY_RE))
          Commit.new(
            graph: unicode_graph(gm[1]),
            hash: "",
            author: "",
            date: "",
            subject: "",
            merge: false,
            graph_only: true,
            decorations: prev_decorations,
            deco_line: false
          )
        end
      end
    end

    def self.parse_decorations(raw)
      raw.strip
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
