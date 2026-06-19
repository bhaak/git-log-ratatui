# frozen_string_literal: true

require "ratatui_ruby"

module GitLogRatatui
  class App
    def initialize(path = ".")
      @path = File.expand_path(path)
      @all_commits = Git.commits(@path)
      @search_query = ""
      @selected_index = 0
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

    def filtered_commits
      return @all_commits if @search_query.empty?

      query = @search_query.downcase
      @all_commits.select do |c|
        c.hash.downcase.include?(query) ||
          c.author.downcase.include?(query) ||
          c.date.downcase.include?(query) ||
          c.subject.downcase.include?(query)
      end
    end

    def render
      commits = filtered_commits
      @selected_index = 0 if commits.any? && @selected_index >= commits.length
      @selected_index = nil if commits.empty?

      @tui.draw do |frame|
        search_area, table_area, controls_area = split_layout(frame.area)

        render_search(frame, search_area)
        render_table(frame, table_area, commits)
        render_controls(frame, controls_area, commits)
      end
    end

    def split_layout(area)
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

    def render_search(frame, area)
      prefix = " Search: "
      search_text = "#{prefix}#{@search_query}"

      widget = @tui.paragraph(
        text: search_text,
        block: @tui.block(
          title: "Git Log — #{@path}",
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
        @tui.text_span(content: "up/down", style: hotkey),
        @tui.text_span(content: ": j/k / arrows  "),
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
      in { type: :key, code: "esc" }
        @search_query = ""
        @selected_index = 0
        nil
      in { type: :key, code: "backspace" }
        return nil if @search_query.empty?

        @search_query = @search_query[0...-1]
        @selected_index = 0
        nil
      in { type: :key, code: "down" } | { type: :key, code: "j" }
        move_selection(1)
      in { type: :key, code: "up" } | { type: :key, code: "k" }
        move_selection(-1)
      in { type: :key, code:, modifiers: [] }
        if code.length == 1
          @search_query += code
          @selected_index = 0
        end
        nil
      else
        nil
      end
    end

    def move_selection(direction)
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
