import datetime

from ..utils import find_next_episode
from ..utils import numeric_ranges
from ..utils import numeric_range
from ..format import short_episode
from ..format import format_airdate
from ..format import format_days


def episode_key(now):
    def __episode_key(item):
        season, episode, airdate = item

        if episode is None or airdate is None:
            return 0

        return -((now - airdate).days)

    return __episode_key


def action(ns):
    before_color = ns.t.green
    inside_color = ns.t.yellow
    outside_color = ns.t.red
    all_seen_color = ns.t.magenta

    before, after = ns.range

    # prevent excessive time fetching.
    now = datetime.datetime.now()

    episode_sort_key = episode_key(now)

    next_episodes = list()
    all_seen = list()

    print ns.t.bold_magenta(u"Next episodes to watch out for")

    if not ns.all:
        print u"Airing within {0} and within {1}".format(
            before_color(format_days(before)),
            inside_color(format_days(after)))
    else:
        print (u"Airing within {0}, within {1}, "
               "outside {2} and {3}").format(
                   before_color(format_days(before)),
                   inside_color(format_days(after)),
                   outside_color(format_days(after)),
                   all_seen_color("never")
               )

    print u""

    for series in ns.series.list_series():
        episodes = ns.series.get_episodes(series)

        if episodes is None:
            print ns.t.bold_red(u"episodes not synced: {0}".format(
                series['series_name']))
            continue

        result = find_next_episode(
            episodes, ns.series.is_episode_watched,
            ignored_seasons=ns.ignore)

        if result is None:
            all_seen.append(series)
            continue

        next_episode, next_airdate = result
        next_episodes.append((series, next_episode, next_airdate))

    next_episodes = sorted(next_episodes, key=episode_sort_key)

    for series, episode, airdate in next_episodes:
        delta_days = abs((now - airdate).days)

        if after is not None and delta_days > after:
            if not ns.all:
                continue
            else:
                color = outside_color
        elif before is None or delta_days < before:
            color = before_color
        else:
            color = inside_color

        print color(u"{2} - {0} {1}".format(
            series['series_name'],
            short_episode(episode),
            format_airdate(episode['first_aired'], now=now)))

    if ns.all:
        for series in all_seen:
            print all_seen_color(u"never - {0}".format(
                series['series_name']))
            continue

    return 0


def setup(parser):
    parser.add_argument(
        '--ignore',
        '-i',
        metavar="<ranges>",
        help="Specify a set of seasons to ignore, defaults to '0'",
        default=set([0]),
        type=numeric_ranges,
    )

    parser.add_argument(
        '--all',
        '-a',
        help=("Display all series episodes, regardless of when they are "
              "airing."),
        default=False,
        action='store_const',
        const=True,
    )

    parser.add_argument(
        '--range',
        help=("Specify how many days ago are relevant, these will show up in "
              "a different color, default: 30"),
        default=(30, 90),
        type=numeric_range,
    )

    parser.set_defaults(action=action)
