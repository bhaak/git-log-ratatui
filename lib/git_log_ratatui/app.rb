# frozen_string_literal: true

require "ratatui_ruby"

module GitLogRatatui
  class App
    def initialize(path = ".")
      @path = File.expand_path(path)
      @branches = Git.branches(@path)
      @branch_index = 0
      @all_commits = Git.commits(@path)
      @search_query = ""
      @selected_index = 0
      @focus = :commits
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

    def branch_names
      ["All Branches"] + @branches
    end

    def selected_branch
      return nil if @branch_index == 0

      @branches[@branch_index - 1]
    end

    def filtered_commits
      query = @search_query.downcase
      commits = @all_commits

      if query.empty?
        commits
      else
        commits.select do |c|
          c.hash.downcase.include?(query) ||
            c.author.downcase.include?(query) ||
            c.date.downcase.include?(query) ||
            c.subject.downcase.include?(query)
        end
      end
    end

    def render
      commits = filtered_commits
      @selected_index = 0 if commits.any? && @selected_index >= commits.length
      @selected_index = nil if commits.empty?

      @tui.draw do |frame|
        branch_area, right_area = horizontal_split(frame.area)
        search_area, table_area, controls_area = vertical_split(right_area)

        render_branches(frame, branch_area)
        render_search(frame, search_area)
        render_table(frame, table_area, commits)
        render_controls(frame, controls_area, commits)
      end
    end

    def horizontal_split(area)
      @tui.layout_split(
        area,
        direction: :horizontal,
        constraints: [
          @tui.constraint_percentage(20),
          @tui.constraint_percentage(80)
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
          @tui.constraint_length(3)
        ]
      )
    end

    def render_branches(frame, area)
      names = branch_names
      items = names.each_with_index.map do |name, i|
        if i == @branch_index && @focus == :branches
          @tui.list_item(content: name, style: @tui.style(bg: :white, fg: :black))
        elsif i == @branch_index
          @tui.list_item(content: name, style: @tui.style(fg: :cyan))
        else
          @tui.list_item(content: name)
        end
      end

      list = @tui.list(
        items: items,
        selected_index: @branch_index,
        block: @tui.block(title: "Branches", borders: [:all]),
        highlight_style: @tui.style(bg: :white, fg: :black),
        highlight_symbol: "> "
      )

      frame.render_widget(list, area)
    end

    def render_search(frame, area)
      branch_label = selected_branch || "all"
      prefix = " Search: "
      search_text = "#{prefix}#{@search_query}"

      widget = @tui.paragraph(
        text: search_text,
        block: @tui.block(
          title: "Git Log — #{@path} [#{branch_label}]",
          borders: [:all],
          border_style: { fg: "cyan" }
        )
      )

      frame.render_widget(widget, area)
    end

    def render_table(frame, area, commits)
      return render_empty_table(frame, area) if commits.empty?

      rows = commits.map do |c|
        @tui.table_row(
          cells: [
            @tui.table_cell(content: c.hash[0, 8]),
            @tui.table_cell(content: c.author),
            @tui.table_cell(content: c.date),
            @tui.table_cell(content: c.subject)
          ]
        )
      end

      widths = [
        @tui.constraint_length(10),
        @tui.constraint_percentage(15),
        @tui.constraint_length(12),
        @tui.constraint_fill(1)
      ]

      highlight_style = @tui.style(bg: :white, fg: :black)

      table = @tui.table(
        header: ["Hash", "Author", "Date", "Subject"],
        rows: rows,
        widths: widths,
        block: @tui.block(borders: [:all]),
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
        @tui.block(borders: [:all], children: [widget]),
        area
      )
    end

    def render_controls(frame, area, commits)
      total = @all_commits.length
      filtered = commits.length
      selected = @selected_index ? @selected_index + 1 : "-"

      hotkey = @tui.style(modifiers: [:bold])

      status_line = @tui.text_line(spans: [
        @tui.text_span(content: "quit", style: hotkey),
        @tui.text_span(content: ": q / C-c  "),
        @tui.text_span(content: "focus", style: hotkey),
        @tui.text_span(content: ": tab / l/h  "),
        @tui.text_span(content: "clear", style: hotkey),
        @tui.text_span(content: ": esc  "),
        @tui.text_span(content: "#{selected}/#{filtered}"),
        @tui.text_span(content: filtered != total ? " (filtered from #{total})" : "")
      ])

      widget = @tui.paragraph(
        text: status_line,
        block: @tui.block(borders: [:all])
      )

      frame.render_widget(widget, area)
    end

    def handle_input
      event = @tui.poll_event

      case event
      in { type: :key, code: "q" } | { type: :key, code: "c", modifiers: ["ctrl"] }
        :quit
      in { type: :key, code: "tab" } | { type: :key, code: "l" }
        @focus = @focus == :branches ? :commits : :branches
        nil
      in { type: :key, code: "h" }
        @focus = @focus == :commits ? :branches : :commits
        nil
      in { type: :key, code: "esc" }
        @search_query = ""
        @selected_index = 0
        nil
      in { type: :key, code: "backspace" }
        handle_backspace
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
      in { type: :key, code:, modifiers: [] }
        handle_char(code) if code.length == 1
        nil
      else
        nil
      end
    end

    def handle_backspace
      return if @search_query.empty?

      @search_query = @search_query[0...-1]
      @selected_index = 0
    end

    def handle_down
      if @focus == :branches
        @branch_index = (@branch_index + 1) % branch_names.length
      else
        move_commit_selection(1)
      end
    end

    def handle_up
      if @focus == :branches
        @branch_index -= 1
        @branch_index = branch_names.length - 1 if @branch_index.negative?
      else
        move_commit_selection(-1)
      end
    end

    def handle_enter
      return unless @focus == :branches

      branch = selected_branch
      @all_commits = Git.commits(@path, branch: branch)
      @selected_index = 0
      @search_query = ""
      @focus = :commits
    end

    def handle_char(char)
      @search_query += char
      @selected_index = 0
    end

    def move_commit_selection(direction)
      commits = filtered_commits
      return if commits.empty?

      new_index = (@selected_index || 0) + direction

      if new_index.negative?
        new_index = commits.length - 1
      elsif new_index >= commits.length
        new_index = 0
      end

      @selected_index = new_index
    end
  end
end
