def action(ns):
    print ns.term.bold_magenta("Series you are currently watching")

    series_list = ns.series.list_series()

    if len(series_list):
        for series in sorted(series_list, key=lambda s: s['series_name']):
            episodes = ns.series.get_episodes(series)

            if episodes is None:
                episodes_count = 'n/a'
            else:
                episodes_count = u"episodes: {0}".format(len(episodes))

            print ns.term.bold_cyan(
                u"{0[id]: 8}: {0[series_name]} ({1})".format(
                    series, episodes_count))
    else:
        print ns.term.bold_red("You are not watching any series")

    return 0


def setup(parser):
    parser.set_defaults(action=action)
