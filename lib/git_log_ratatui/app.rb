# frozen_string_literal: true

require "ratatui_ruby"

module GitLogRatatui
  class App
    FOCUS_BORDER = { fg: "magenta" }.freeze
    PANELS = %i[branches search commits diff].freeze

    def initialize(path = ".")
      @path = File.expand_path(path)
      @branches = Git.branches(@path)
      @branch_index = 0
      @all_commits = Git.commits(@path)
      @search_query = ""
      @cursor_pos = 0
      @selected_index = 0
      @focus = :search
      @diff_lines = []
      @diff_scroll = 0
      @branch_width_pct = 20
      @diff_height_pct = 35
      @dragging = nil
      @selected_file_index = nil
      @branch_tree = build_branch_tree
      @branch_expanded = {}
    end

    def run
      RatatuiRuby.run do |tui|
        @tui = tui
        loop do
          render
          break if handle_input == :quit
        end
      end
    end

    private

    TreeItem = Data.define(:name, :depth, :expandable, :expanded, :branch, :full_path, :tree_prefix)

    def build_branch_tree
      tree = { children: {} }
      @branches.each do |branch|
        parts = branch.split("/")
        current = tree
        parts.each do |part|
          current[:children][part] ||= { children: {} }
          current = current[:children][part]
        end
        current[:branch_name] = branch
      end
      tree
    end

    def flat_branch_items
      items = [TreeItem.new(name: "All Branches", depth: 0, expandable: false, expanded: false, branch: nil, full_path: "", tree_prefix: "")]

      flush = lambda do |node, depth, parent_path, ancestors|
        names = node[:children].keys
        sorted = names.partition { |n| n == "main" || n == "master" }.flatten
        sorted.each_with_index do |key, idx|
          child = node[:children][key]
          is_leaf = child[:children].empty?
          full_path = parent_path.empty? ? key : "#{parent_path}/#{key}"
          expanded = @branch_expanded[full_path] || false
          is_last = idx == sorted.length - 1

          prefix = ancestors.map { |a| a ? "  " : "│ " }.join
          connector = is_last ? "└" : "├"
          items << TreeItem.new(
            name: key,
            depth: depth,
            expandable: !is_leaf,
            expanded: expanded,
            branch: is_leaf ? child[:branch_name] : nil,
            full_path: full_path,
            tree_prefix: "#{prefix}#{connector}"
          )

          if !is_leaf && expanded
            flush.call(child, depth + 1, full_path, ancestors + [is_last])
          end
        end
      end

      flush.call(@branch_tree, 1, "", [])
      items
    end

    def selected_branch
      items = flat_branch_items
      return nil unless @branch_index && @branch_index < items.length

      items[@branch_index].branch
    end

    def filtered_commits
      query = @search_query.downcase
      return @all_commits if query.empty?

      @all_commits.select do |c|
        c.graph_only ||
          c.hash.downcase.include?(query) ||
          c.author.downcase.include?(query) ||
          c.date.downcase.include?(query) ||
          c.subject.downcase.include?(query)
      end
    end

    def clamp_cursor
      @cursor_pos = @cursor_pos.clamp(0, @search_query.length)
    end

    def selected_commit
      commits = filtered_commits
      return nil unless @selected_index && @selected_index < commits.length

      c = commits[@selected_index]
      c.graph_only ? nil : c
    end

    FILE_HEADER_STYLE = { fg: "cyan", modifiers: [:bold] }.freeze
    FILE_ENTRY_STYLE = { fg: "blue" }.freeze

    def refresh_diff
      commit = selected_commit
      raw_diff = commit ? Git.diff(@path, commit.hash) : []
      @commit_info = commit ? Git.commit_info(@path, commit.hash) : nil

      if @commit_info
        @metadata_lines = build_commit_metadata
        @diff_lines = raw_diff
        @file_entries, @file_lines = build_file_list
        @metadata_count = @metadata_lines.length
        @file_count = @file_lines.length
      else
        @metadata_lines = []
        @file_lines = []
        @diff_lines = raw_diff
        @file_entries = []
        @metadata_count = 0
        @file_count = 0
      end
      @diff_scroll = 0
      @selected_file_index = @file_entries.empty? ? nil : 0
    end

    def build_file_list
      entries = []
      @diff_lines.each_with_index do |line, idx|
        if (md = line.match(/\Adiff --git a\/(.*?) b\/(.*)/))
          filename = md[2]
          entries << { name: filename, diff_line: idx }
        end
      end
      return [[], []] if entries.empty?

      list = [["", "Changed files:", FILE_HEADER_STYLE]]
      entries.each { |e| list << ["", e[:name], FILE_ENTRY_STYLE] }
      list << ["", "", {}]
      [entries, list]
    end

    SUBJECT_STYLE = { fg: "white", modifiers: [:bold] }.freeze
    LABEL_STYLE = { fg: "dark gray" }.freeze
    LABEL_WIDTH = 15

    def build_commit_metadata
      info = @commit_info
      pairs = []

      pairs << ["Subject:", info.subject, SUBJECT_STYLE]
      pairs << ["Hash:", info.hash, {}]
      parents = info.parents.empty? ? "—" : info.parents
      pairs << ["Parents:", parents, {}]

      author = "#{info.author_name} <#{info.author_email}>"
      pairs << ["Author:", author, {}]
      pairs << ["Author date:", info.author_date, {}]

      same_committer = info.author_name == info.committer_name && info.author_email == info.committer_email
      unless same_committer
        committer = "#{info.committer_name} <#{info.committer_email}>"
        pairs << ["Committer:", committer, {}]
      end

      unless same_committer && info.author_date == info.committer_date
        pairs << ["Committer date:", info.committer_date, {}]
      end

      pairs << ["", "", {}]
      pairs
    end

    def render
      clamp_cursor
      commits = filtered_commits
      @selected_index = 0 if commits.any? && @selected_index >= commits.length
      @selected_index = nil if commits.empty?

      prev_commit = @last_selected_index
      @last_selected_index = @selected_index
      refresh_diff if @selected_index != prev_commit

      @tui.draw do |frame|
        branch_area, right_area = horizontal_split(frame.area)
        search_area, table_area, diff_area, controls_area = vertical_split(right_area)

        @areas = {
          branches: branch_area,
          search: search_area,
          commits: table_area,
          diff: diff_area
        }

        render_branches(frame, branch_area)
        render_search(frame, search_area)
        render_table(frame, table_area, commits)
        render_diff(frame, diff_area)
        render_controls(frame, controls_area, commits)
      end
    end

    def horizontal_split(area)
      @tui.layout_split(
        area,
        direction: :horizontal,
        constraints: [
          @tui.constraint_percentage(@branch_width_pct),
          @tui.constraint_percentage(100 - @branch_width_pct)
        ]
      )
    end

    def vertical_split(area)
      @tui.layout_split(
        area,
        direction: :vertical,
        constraints: [
          @tui.constraint_length(3),
          @tui.constraint_fill(1),
          @tui.constraint_percentage(@diff_height_pct),
          @tui.constraint_length(3)
        ]
      )
    end

    def focused?(panel)
      @focus == panel
    end

    def render_branches(frame, area)
      items = flat_branch_items
      list_items = items.map.with_index do |item, i|
        if item.expandable
          marker = item.expanded ? "▼ " : "▶ "
        else
          marker = "  "
        end

        if i == 0
          display = item.name
        else
          display = "#{item.tree_prefix}#{marker}#{item.name}"
        end
        @tui.list_item(content: display)
      end

      list = @tui.list(
        items: list_items,
        selected_index: @branch_index,
        block: @tui.block(
          title: "Branches",
          borders: [:all],
          border_style: focused?(:branches) ? FOCUS_BORDER : nil
        ),
        highlight_style: @tui.style(bg: :white, fg: :black),
        highlight_symbol: "> "
      )

      frame.render_widget(list, area)
    end

    def render_search(frame, area)
      branch_label = selected_branch || "all"
      prefix = " Search: "

      text = if focused?(:search)
               render_search_with_cursor(prefix)
             else
               @tui.text_line(spans: [
                 @tui.text_span(content: "#{prefix}#{@search_query}")
               ])
             end

      widget = @tui.paragraph(
        text: text,
        block: @tui.block(
          title: "Git Log — #{@path} [#{branch_label}]",
          borders: [:all],
          border_style: focused?(:search) ? FOCUS_BORDER : nil
        )
      )

      frame.render_widget(widget, area)
    end

    def render_search_with_cursor(prefix)
      before = @search_query[0, @cursor_pos]
      at = @search_query[@cursor_pos]
      after = @search_query[@cursor_pos + 1..]

      cursor_style = @tui.style(bg: :white, fg: :black)

      spans = [@tui.text_span(content: prefix)]

      spans << @tui.text_span(content: before) unless before.empty?

      if at
        spans << @tui.text_span(content: at, style: cursor_style)
      else
        spans << @tui.text_span(content: " ", style: cursor_style)
      end

      spans << @tui.text_span(content: after) if after

      @tui.text_line(spans: spans)
    end

    def render_table(frame, area, commits)
      return render_empty_table(frame, area) if commits.empty?

      max_graph = commits.map { |c| c.graph.length }.max
      graph_width = [max_graph, 4].max

      rows = commits.map do |c|
        graph = c.graph.ljust(graph_width)
        graph_content = if c.decorations.empty?
                          graph
                        else
                          "#{graph} #{c.decorations}"
                        end

        if c.graph_only
          @tui.table_row(
            cells: [
              @tui.table_cell(content: graph_content),
              @tui.table_cell(content: ""),
              @tui.table_cell(content: ""),
              @tui.table_cell(content: ""),
              @tui.table_cell(content: "")
            ]
          )
        elsif c.merge
          @tui.table_row(
            cells: [
              @tui.table_cell(content: graph_content, style: @tui.style(fg: "yellow")),
              @tui.table_cell(content: c.hash[0, 8]),
              @tui.table_cell(content: c.subject),
              @tui.table_cell(content: c.author),
              @tui.table_cell(content: c.date)
            ]
          )
        else
          @tui.table_row(
            cells: [
              @tui.table_cell(content: graph_content),
              @tui.table_cell(content: c.hash[0, 8]),
              @tui.table_cell(content: c.subject),
              @tui.table_cell(content: c.author),
              @tui.table_cell(content: c.date)
            ]
          )
        end
      end

      widths = [
        @tui.constraint_length(graph_width),
        @tui.constraint_length(10),
        @tui.constraint_fill(1),
        @tui.constraint_percentage(15),
        @tui.constraint_length(18)
      ]

      highlight_style = @tui.style(bg: :white, fg: :black)

      table = @tui.table(
        header: ["Graph", "Hash", "Subject", "Author", "Date"],
        rows: rows,
        widths: widths,
        block: @tui.block(
          borders: [:all],
          border_style: focused?(:commits) ? FOCUS_BORDER : nil
        ),
        selected_row: @selected_index,
        row_highlight_style: highlight_style,
        highlight_symbol: "> ",
        column_spacing: 1
      )

      frame.render_widget(table, area)
    end

    def render_empty_table(frame, area)
      msg = @all_commits.empty? ? "No commits found in this repository." : "No commits match your search."

      widget = @tui.center(
        child: @tui.paragraph(text: msg, alignment: :center),
        width_percent: 80,
        height_percent: 80
      )

      frame.render_widget(
        @tui.block(
          borders: [:all],
          border_style: focused?(:commits) ? FOCUS_BORDER : nil,
          children: [widget]
        ),
        area
      )
    end

    ADD_LINE = { fg: "green" }.freeze
    DEL_LINE = { fg: "red" }.freeze
    HUNK_STYLE = { fg: "cyan" }.freeze
    META_STYLE = { fg: "yellow" }.freeze

    def add_word_style
      @tui.style(bg: RatatuiRuby::Style::Color.hex(0x003300), fg: "green")
    end

    def del_word_style
      @tui.style(bg: RatatuiRuby::Style::Color.hex(0x330000), fg: "red")
    end

    def render_diff(frame, area)
      commit = selected_commit
      title = commit ? "Diff — #{commit.hash[0, 8]}" : "Diff"

      all_lines = @metadata_lines + @file_lines + @diff_lines

      if all_lines.empty?
        msg = commit ? "No changes in this commit." : "Select a commit to view diff."
        widget = @tui.paragraph(
          text: msg,
          alignment: :center,
          block: @tui.block(
            title: title,
            borders: [:all],
            border_style: focused?(:diff) ? FOCUS_BORDER : nil
          )
        )
        frame.render_widget(widget, area)
        return
      end

      inner_height = area.height - 2
      return if inner_height <= 0

      visible_lines = all_lines[@diff_scroll, inner_height] || []

      styled_lines = visible_lines.each_with_index.map do |line, vi|
        actual_idx = @diff_scroll + vi
        styled_diff_line(line, actual_idx)
      end

      total_lines = all_lines.length
      scroll_note = if total_lines > inner_height
                      " lines #{@diff_scroll + 1}-#{[@diff_scroll + inner_height, total_lines].min}/#{total_lines}"
                    else
                      ""
                    end

      widget = @tui.paragraph(
        text: styled_lines,
        block: @tui.block(
          title: "#{title}#{scroll_note}",
          borders: [:all],
          border_style: focused?(:diff) ? FOCUS_BORDER : nil
        )
      )

      frame.render_widget(widget, area)
    end

    def styled_diff_line(line, idx)
      if idx < @metadata_count
        styled_metadata_line(line, idx)
      elsif idx < @metadata_count + @file_count
        styled_metadata_line(line, idx)
      else
        styled_code_line(line, idx - @metadata_count - @file_count)
      end
    end

    def styled_metadata_line(line, idx)
      label, value, value_style = line

      if label.empty? && value.empty?
        return @tui.text_line(spans: [@tui.text_span(content: "")])
      end

      if label.empty?
        entry_style = value_style
        if selected_file_entry?(idx)
          selected_fg = value_style[:fg] || "black"
          entry_style = { bg: "white", fg: selected_fg }
        end

        value_span = if entry_style.empty?
                       @tui.text_span(content: value)
                     else
                       @tui.text_span(content: value, style: @tui.style(**entry_style))
                     end
        return @tui.text_line(spans: [value_span])
      end

      label_text = label.ljust(LABEL_WIDTH)
      label_span = @tui.text_span(content: label_text, style: @tui.style(**LABEL_STYLE))
      value_span = if value_style.empty?
                     @tui.text_span(content: value)
                   else
                     @tui.text_span(content: value, style: @tui.style(**value_style))
                   end

      @tui.text_line(spans: [label_span, value_span])
    end

    def selected_file_entry?(idx)
      return false if @file_entries.empty? || @selected_file_index.nil?

      file_start = @metadata_count + 1
      file_idx = idx - file_start
      file_idx >= 0 && file_idx < @file_entries.length && file_idx == @selected_file_index
    end

    def styled_code_line(line, idx)
      meta_style = diff_meta_style(line)
      return @tui.text_line(spans: [@tui.text_span(content: line, style: @tui.style(**meta_style))]) if meta_style

      case line[0] || ""
      when "+"
        styled_change_line(line, idx + @metadata_count, :add)
      when "-"
        styled_change_line(line, idx + @metadata_count, :del)
      else
        @tui.text_line(spans: [@tui.text_span(content: line)])
      end
    end

    def styled_change_line(line, idx, type)
      pair_line = find_pair_line(idx, type)
      return plain_styled_line(line, type) unless pair_line

      old_words, new_words = type == :del ? [tokenize(line), tokenize(pair_line)] : [tokenize(pair_line), tokenize(line)]
      lcs_result = lcs_words(old_words, new_words)
      word_highlight_spans(old_words, new_words, lcs_result, type)
    end

    def find_pair_line(idx, type)
      return nil if idx < 0 || idx >= @diff_lines.length
      return nil unless @diff_lines[idx].start_with?(type == :del ? "-" : "+")

      if type == :del
        pair_idx = idx + 1
        while pair_idx < @diff_lines.length
          break if @diff_lines[pair_idx].start_with?("+") || @diff_lines[pair_idx].start_with?(" ")
          pair_idx += 1
        end
        return nil if pair_idx >= @diff_lines.length || !@diff_lines[pair_idx].start_with?("+")

        @diff_lines[pair_idx]
      else
        pair_idx = idx - 1
        while pair_idx >= 0
          break if @diff_lines[pair_idx].start_with?("-") || @diff_lines[pair_idx].start_with?(" ")
          pair_idx -= 1
        end
        return nil if pair_idx < 0 || !@diff_lines[pair_idx].start_with?("-")

        @diff_lines[pair_idx]
      end
    end

    def plain_styled_line(line, type)
      line_style = @tui.style(**(type == :add ? ADD_LINE : DEL_LINE))
      @tui.text_line(spans: [@tui.text_span(content: line)], style: line_style)
    end

    def tokenize(line)
      line[1..].scan(/[\[\]\(\)\{\}]|[^\[\]\(\)\{\}\s]+|\s+/)
    end

    def lcs_words(a, b)
      m = a.length
      n = b.length
      dp = Array.new(m + 1) { Array.new(n + 1, 0) }

      (1..m).each do |i|
        (1..n).each do |j|
          dp[i][j] = if a[i - 1] == b[j - 1]
                       dp[i - 1][j - 1] + 1
                     else
                       [dp[i - 1][j], dp[i][j - 1]].max
                     end
        end
      end

      old_set = Set.new
      new_set = Set.new
      i = m
      j = n
      while i > 0 && j > 0
        if a[i - 1] == b[j - 1]
          old_set.add(i - 1)
          new_set.add(j - 1)
          i -= 1
          j -= 1
        elsif dp[i - 1][j] > dp[i][j - 1]
          i -= 1
        else
          j -= 1
        end
      end

      { old: old_set, new: new_set }
    end

    def word_highlight_spans(old_words, new_words, lcs, type)
      words = type == :del ? old_words : new_words
      unchanged_set = type == :del ? lcs[:old] : lcs[:new]
      line_style_key = type == :del ? DEL_LINE : ADD_LINE
      word_style = type == :del ? del_word_style : add_word_style

      line_style = @tui.style(**line_style_key)

      spans = [@tui.text_span(content: type == :del ? "-" : "+")]
      words.each_with_index do |word, i|
        if unchanged_set.include?(i)
          spans << @tui.text_span(content: word)
        else
          spans << @tui.text_span(content: word, style: word_style)
        end
      end

      @tui.text_line(spans: spans, style: line_style)
    end

    def diff_meta_style(line)
      case line
      when /\A@@/ then HUNK_STYLE
      when /\A(diff|index|---|\+\+\+)/ then META_STYLE
      end
    end

    def render_controls(frame, area, commits)
      total = @all_commits.length
      filtered = commits.length
      selected = @selected_index ? @selected_index + 1 : "-"

      key_style = @tui.style(fg: "yellow")
      dim = @tui.style(fg: "dark gray")

      pairs = [["q", "Quit"], ["tab", "Focus"]]

      case @focus
      when :branches
        pairs += [["↑↓", "Nav"], ["→←", "Expand"], ["␣", "Toggle"], ["enter", "Load"]]
      when :search
        pairs += [["esc", "Clear"]]
      when :commits
        pairs += [["↑↓", "Nav"]]
      when :diff
        pairs += [["↑↓", "File"], ["enter", "Jump"], ["n/p", "Next/prev"], ["home/end", "Top/bottom"]]
      end

      count_text = "#{selected}/#{filtered}#{filtered != total ? " (filtered from #{total})" : ""}"

      spans = []
      pairs.each do |key, desc|
        spans << @tui.text_span(content: "<", style: dim)
        spans << @tui.text_span(content: key, style: key_style)
        spans << @tui.text_span(content: ": ", style: dim)
        spans << @tui.text_span(content: desc)
        spans << @tui.text_span(content: ">  ", style: dim)
      end

      spans << @tui.text_span(content: count_text, style: dim)

      widget = @tui.paragraph(
        text: @tui.text_line(spans: spans),
        block: @tui.block(title: "Help", borders: [:all])
      )

      frame.render_widget(widget, area)
    end

    def handle_input
      event = @tui.poll_event

      case event
      in { type: :key, code: "q" } | { type: :key, code: "c", modifiers: ["ctrl"] }
        :quit
      in { type: :key, code: "z", modifiers: ["ctrl"] }
        suspend
        nil
      in { type: :key, code: "tab" } | { type: :key, code: "l" }
        cycle_focus(1)
        nil
      in { type: :key, code: "tab", modifiers: ["shift"] } | { type: :key, code: "h" }
        cycle_focus(-1)
        nil
      in { type: :key, code: "a", modifiers: ["ctrl"] }
        handle_home
        nil
      in { type: :key, code: "e", modifiers: ["ctrl"] }
        handle_end
        nil
      in { type: :key, code: "left", modifiers: ["ctrl"] }
        handle_word_left
        nil
      in { type: :key, code: "right", modifiers: ["ctrl"] }
        handle_word_right
        nil
      in { type: :key, code: "esc" }
        handle_esc
        nil
      in { type: :key, code: "backspace" }
        handle_backspace
        nil
      in { type: :key, code: "delete" }
        handle_delete
        nil
      in { type: :key, code: "left" }
        handle_left
        nil
      in { type: :key, code: "right" }
        handle_right
        nil
      in { type: :key, code: "home" }
        handle_home
        nil
      in { type: :key, code: "end" }
        handle_end
        nil
      in { type: :key, code: "down" } | { type: :key, code: "j" }
        handle_down
        nil
      in { type: :key, code: "up" } | { type: :key, code: "k" }
        handle_up
        nil
      in { type: :key, code: "enter" }
        handle_enter
        nil
      in { type: :key, code: " " }
        handle_space
        nil
      in { type: :key, code: "n" } if @focus == :diff
        handle_next_file
        nil
      in { type: :key, code: "p" } if @focus == :diff
        handle_prev_file
        nil
      in { type: :key, code:, modifiers: [] }
        handle_char(code) if @focus == :search && code.length == 1
        nil
      in { type: :mouse }
        handle_mouse(event)
        nil
      in { type: :paste }
        handle_paste(event)
        nil
      else
        nil
      end
    end

    def cycle_focus(direction)
      idx = PANELS.index(@focus)
      new_idx = (idx + direction) % PANELS.length
      @focus = PANELS[new_idx]
    end

    def suspend
      RatatuiRuby.restore_terminal
      Process.kill("TSTP", Process.pid)
      RatatuiRuby.init_terminal
    end

    def handle_esc
      return unless @focus == :search

      @search_query = ""
      @cursor_pos = 0
      @selected_index = 0
    end

    def handle_backspace
      return unless @focus == :search
      return if @search_query.empty? || @cursor_pos == 0

      @search_query = @search_query[0...@cursor_pos - 1] + @search_query[@cursor_pos..]
      @cursor_pos -= 1
      @selected_index = 0
    end

    def handle_delete
      return unless @focus == :search
      return if @cursor_pos >= @search_query.length

      @search_query = @search_query[0...@cursor_pos] + @search_query[@cursor_pos + 1..]
      @selected_index = 0
    end

    def handle_left
      if @focus == :branches
        branch_toggle(false)
      elsif @focus == :search
        @cursor_pos -= 1 if @cursor_pos > 0
      end
    end

    def handle_right
      if @focus == :branches
        branch_toggle(true)
      elsif @focus == :search
        @cursor_pos += 1 if @cursor_pos < @search_query.length
      end
    end

    def handle_home
      if @focus == :diff
        @diff_scroll = 0
      elsif @focus == :search
        @cursor_pos = 0
      end
    end

    def handle_end
      if @focus == :diff
        total = @metadata_count + @file_count + @diff_lines.length
        @diff_scroll = [total - 1, 0].max
      elsif @focus == :search
        @cursor_pos = @search_query.length
      end
    end

    def handle_word_left
      return unless @focus == :search

      pos = @cursor_pos - 1
      pos -= 1 while pos > 0 && @search_query[pos] == " "
      pos -= 1 while pos >= 0 && @search_query[pos] != " "
      @cursor_pos = pos + 1
    end

    def handle_word_right
      return unless @focus == :search

      pos = @cursor_pos
      pos += 1 while pos < @search_query.length && @search_query[pos] != " "
      pos += 1 while pos < @search_query.length && @search_query[pos] == " "
      @cursor_pos = pos
    end

    def handle_down
      case @focus
      when :branches
        @branch_index = (@branch_index + 1) % flat_branch_items.length
      when :commits
        move_commit_selection(1)
      when :diff
        if file_list_visible?
          select_next_file
        else
          scroll_diff(1)
        end
      end
    end

    def handle_up
      case @focus
      when :branches
        @branch_index -= 1
        @branch_index = flat_branch_items.length - 1 if @branch_index.negative?
      when :commits
        move_commit_selection(-1)
      when :diff
        if file_list_visible?
          select_prev_file
        else
          scroll_diff(-1)
        end
      end
    end

    def file_list_visible?
      @file_entries.any? && @diff_scroll < @metadata_count + @file_count
    end

    def select_next_file
      return if @file_entries.empty?

      if @selected_file_index.nil? || @selected_file_index >= @file_entries.length - 1
        @selected_file_index = 0
      else
        @selected_file_index += 1
      end
    end

    def select_prev_file
      return if @file_entries.empty?

      if @selected_file_index.nil? || @selected_file_index <= 0
        @selected_file_index = @file_entries.length - 1
      else
        @selected_file_index -= 1
      end
    end

    def handle_enter
      case @focus
      when :branches
        items = flat_branch_items
        return unless @branch_index && @branch_index < items.length

        item = items[@branch_index]
        if item.expandable
          @branch_expanded[item.full_path] = !item.expanded
        elsif @branch_index == 0
          load_branch_commits(nil)
        elsif item.branch
          load_branch_commits(item.branch)
        end
      when :diff
        return if @file_entries.empty? || @selected_file_index.nil?

        entry = @file_entries[@selected_file_index]
        return unless entry

        @diff_scroll = @metadata_count + @file_count + entry[:diff_line]
      end
    end

    def branch_toggle(expand)
      items = flat_branch_items
      return unless @branch_index && @branch_index < items.length

      item = items[@branch_index]
      return unless item.expandable
      return if item.expanded == expand

      @branch_expanded[item.full_path] = expand
    end

    def load_branch_commits(branch)
      @all_commits = Git.commits(@path, branch: branch)
      @selected_index = 0
      @search_query = ""
      @cursor_pos = 0
      @focus = :commits
    end

    def handle_next_file
      return unless @focus == :diff
      return if @file_entries.empty?

      if @selected_file_index.nil? || @selected_file_index >= @file_entries.length - 1
        @selected_file_index = 0
        @diff_scroll = @metadata_count + @file_count + @file_entries[0][:diff_line]
      else
        @selected_file_index += 1
        @diff_scroll = @metadata_count + @file_count + @file_entries[@selected_file_index][:diff_line]
      end
    end

    def handle_prev_file
      return unless @focus == :diff
      return if @file_entries.empty?

      if @selected_file_index.nil? || @selected_file_index <= 0
        @diff_scroll = 0
        @selected_file_index = 0
      else
        @selected_file_index -= 1
        @diff_scroll = @metadata_count + @file_count + @file_entries[@selected_file_index][:diff_line]
      end
    end

    def handle_space
      return unless @focus == :branches

      items = flat_branch_items
      return unless @branch_index && @branch_index < items.length

      item = items[@branch_index]
      return unless item.expandable

      @branch_expanded[item.full_path] = !item.expanded
    end

    def handle_char(char)
      @search_query = @search_query[0...@cursor_pos] + char + @search_query[@cursor_pos..]
      @cursor_pos += 1
      @selected_index = 0
    end

    def handle_paste(event)
      text = event.content
      return if text.empty?

      @search_query = text
      @cursor_pos = text.length
      @selected_index = 0
      @focus = :search
    end

    def move_commit_selection(direction)
      commits = filtered_commits
      return if commits.empty?

      attempts = 0
      loop do
        new_index = (@selected_index || 0) + direction

        if new_index.negative?
          new_index = commits.length - 1
        elsif new_index >= commits.length
          new_index = 0
        end

        @selected_index = new_index
        break unless commits[new_index].graph_only
        break if (attempts += 1) >= commits.length
      end
    end

    def scroll_diff(direction)
      total = @metadata_count + @file_count + @diff_lines.length
      return if total == 0

      @diff_scroll += direction
      @diff_scroll = @diff_scroll.clamp(0, total - 1)
    end

    def handle_mouse(event)
      if event.pressed? && event.left?
        border = find_resize_border(event.x, event.y)
        if border
          @dragging = border
        else
          @dragging = nil
          handle_mouse_click(event)
        end
      elsif (event.drag? || event.moved?) && @dragging
        handle_resize_drag(event)
      elsif event.released?
        @dragging = nil
        nil
      elsif event.scroll?
        handle_mouse_scroll(event)
      end
    end

    def find_resize_border(x, y)
      return nil unless @areas && @areas[:branches]

      branch_right = @areas[:branches].right
      commits_bottom = @areas[:commits]&.bottom
      diff_top = @areas[:diff]&.top

      if (x - branch_right).abs <= 2
        :vertical
      elsif commits_bottom && diff_top && (y - commits_bottom).abs <= 2
        :horizontal
      end
    end

    def handle_resize_drag(event)
      case @dragging
      when :vertical
        total = @areas[:branches].width + @areas[:search].width
        new_width = event.x.clamp(10, total - 20)
        @branch_width_pct = (new_width.to_f / total * 100).clamp(10, 40)
      when :horizontal
        total = @areas[:branches].height
        border_y = event.y.clamp(5, total - 5)
        @diff_height_pct = ((total - border_y - 3).to_f / total * 100).clamp(10, 65)
      end
      nil
    end

    def handle_mouse_click(event)
      panel = hit_test(event.x, event.y)
      return unless panel

      @focus = panel

      case panel
      when :branches then handle_branch_click(event)
      when :commits then handle_commit_click(event)
      when :diff then handle_diff_click(event)
      end
    end

    def handle_mouse_scroll(event)
      panel = hit_test(event.x, event.y)
      return unless panel

      direction = event.scroll_down? ? 1 : -1

      case panel
      when :branches
        @branch_index = (@branch_index + direction) % flat_branch_items.length
      when :commits
        move_commit_selection(direction)
      when :diff
        scroll_diff(direction)
      end
    end

    def hit_test(x, y)
      PANELS.find { |panel| @areas[panel]&.contains?(x, y) }
    end

    def handle_branch_click(event)
      area = @areas[:branches]
      return unless area

      item_y = event.y - area.y - 1
      return if item_y.negative?

      items = flat_branch_items
      return if item_y >= items.length

      @branch_index = item_y
      item = items[item_y]

      if item.expandable
        @branch_expanded[item.full_path] = !item.expanded
      elsif item_y == 0
        load_branch_commits(nil)
      elsif item.branch
        load_branch_commits(item.branch)
      end
    end

    def handle_commit_click(event)
      area = @areas[:commits]
      return unless area

      row_y = event.y - area.y - 2
      return if row_y.negative?

      commits = filtered_commits
      return if commits.empty? || row_y >= commits.length

      return if commits[row_y].graph_only

      @selected_index = row_y
    end

    def handle_diff_click(event)
      area = @areas[:diff]
      return unless area

      content_line = (event.y - area.y - 1) + @diff_scroll
      file_start = @metadata_count + 1
      file_end = file_start + @file_entries.length - 1

      if content_line >= file_start && content_line <= file_end
        file_idx = content_line - file_start
        return if file_idx >= @file_entries.length

        @selected_file_index = file_idx
        entry = @file_entries[file_idx]
        @diff_scroll = @metadata_count + @file_count + entry[:diff_line]
      end
    end
  end
end
