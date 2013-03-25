import datetime

from ..utils import parse_datetime


def series_name_key(s):
    return s['series_name']


def has_aired_filter(now):
    def __has_aired(e):
        if not e['first_aired']:
            return False
        return now >= parse_datetime(e['first_aired'])
    return __has_aired


def action(ns):
    print ns.term.bold_magenta("Series you are currently watching")

    now = datetime.datetime.now()

    has_aired = has_aired_filter(now)

    series_list = ns.series.list_series()

    episodes_description = (
        u"{t.green}{0}{t.normal}/"
        u"{t.yellow}{1}{t.normal}/"
        u"{t.red}{2}{t.normal}"
    ).format(
        "seen", "aired", "all",
        t=ns.term)

    if series_list:
        for series in sorted(series_list, key=series_name_key):
            episodes = ns.series.get_episodes(series)

            episodes_count = None

            if episodes is not None:
                seen_episodes = len(filter(
                    ns.series.is_episode_watched, episodes))

                aired_episodes = len(filter(has_aired, episodes))

                all_episodes = len(episodes)

                episodes_count = (
                    u"{t.green}{0}{t.normal}/"
                    u"{t.yellow}{1}{t.normal}/"
                    u"{t.red}{2}{t.normal}"
                ).format(
                    seen_episodes, aired_episodes, all_episodes,
                    t=ns.term)

            print ns.term.bold_cyan(
                u"{0[series_name]} (id: {0[id]})".format(series))

            if episodes_count:
                print ns.term.cyan(
                    u"  Episodes ({0}): {1}".format(
                        episodes_description,
                        episodes_count))
    else:
        print ns.term.bold_red("You are not watching any series")

    return 0


def setup(parser):
    parser.set_defaults(action=action)
