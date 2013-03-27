import datetime

from ..utils import find_next_episode
from ..utils import numeric_ranges
from ..format import short_episode
from ..format import format_airdate


def episode_key(now):
    def __episode_key(item):
        season, episode, airdate = item

        if episode is None or airdate is None:
            return 0

        return -((now - airdate).days)

    return __episode_key


def action(ns):
    print ns.t.bold_magenta(u"Next episodes to watch out for")
    print u""

    # prevent excessive time fetching.
    now = datetime.datetime.now()

    episode_sort_key = episode_key(now)

    next_episodes = list()
    all_seen = list()

    for series in ns.series.list_series():
        episodes = ns.series.get_episodes(series)

        if episodes is None:
            print ns.t.bold_red(u"episodes not synced: {0}".format(
                series['series_name']))
            continue

        result = find_next_episode(
            episodes, ns.series.is_episode_watched,
            ignored_seasons=ns.ignored_seasons)

        if result is None:
            all_seen.append(series)
            continue

        next_episode, next_airdate = result
        next_episodes.append((series, next_episode, next_airdate))

    next_episodes = sorted(next_episodes, key=episode_sort_key)

    for series, episode, airdate in next_episodes:
        delta_days = abs((now - airdate).days)

        color = ns.t.bold_white

        if delta_days > ns.relevant_days:
            color = ns.t.white

        print color(u"{2} - {0} {1}".format(
            series['series_name'],
            short_episode(episode),
            format_airdate(episode['first_aired'], now=now)))

    for series in all_seen:
        print ns.t.green(u"{0}: all seen".format(
            series['series_name']))
        continue

    return 0


def setup(parser):
    parser.add_argument(
        '--ignored-seasons', '-i',
        help="Specify a list of seasons to ignore, defaults to '0'",
        default=set([0]),
        type=numeric_ranges,
    )

    parser.add_argument(
        '--relevant-days',
        help=("Specify how many days ago are relevant, these will show up in "
              "a different color, default: 30"),
        default=30,
        type=int,
    )

    parser.set_defaults(action=action)
