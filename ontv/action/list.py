import datetime

from ..utils import has_aired_filter
from ..format import format_episodes_count
from ..format import format_episodes_count_legend


def series_name_key(s):
    return s['series_name']


def action(ns):
    print ns.t.bold_magenta(u"Series you are currently watching")
    print u""

    now = datetime.datetime.now()

    has_aired = has_aired_filter(now)

    series_dao = ns.series

    series_list = ns.series.list_series()

    episodes_legend = format_episodes_count_legend(ns.t)

    if series_list:
        for series in sorted(series_list, key=series_name_key):
            episodes = ns.series.get_episodes(series)

            episodes_count = None

            if episodes is not None:
                episodes_count, _ = format_episodes_count(
                    ns.t, series_dao, has_aired, episodes)

            print ns.t.bold_cyan(
                u"{0[series_name]} (id: {0[id]})".format(series))

            if episodes_count:
                print ns.t.cyan(
                    u"  Episodes ({0}): {1}".format(
                        episodes_legend, episodes_count))
    else:
        print ns.t.bold_red("You are not watching any series")

    return 0


def setup(parser):
    parser.set_defaults(action=action)
