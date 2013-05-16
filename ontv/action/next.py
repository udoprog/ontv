import datetime

from ..utils import find_next_episode
from ..utils import numeric_ranges
from ..utils import numeric_range
from ..utils import with_resource
from ..utils import local_series_finder

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


def filter_episodes(ns, next_episodes, specific, now):
    before, after = ns.range

    for s, episode, airdate in next_episodes:
        delta_days = abs((now - airdate).days)

        if after is not None and delta_days > after:
            if specific or ns.all:
                color = ns.C.range_outside
            else:
                continue
        elif before is None or delta_days < before:
            color = ns.C.range_before
        else:
            color = ns.C.range_inside

        yield color, s, episode


@with_resource(local_series_finder)
def action(ns, series):
    specific = False

    if series is None:
        series = ns.series.list_series()
    else:
        specific = True
        series = [series]

    before, after = ns.range

    # prevent excessive time fetching.
    now = datetime.datetime.now()

    episode_sort_key = episode_key(now)

    next_episodes = list()
    all_seen = list()

    print ns.C.title(u"Next episodes to watch out for")

    if specific or ns.all:
        print (u"Airing within {0}, within {1}, "
               "outside {2} and {3}").format(
                   ns.C.range_before(format_days(before)),
                   ns.C.range_inside(format_days(after)),
                   ns.C.range_outside(format_days(after)),
                   ns.C.all_seen("never")
               )
    else:
        print u"Airing within {0} and within {1}".format(
            ns.C.range_before(format_days(before)),
            ns.C.range_inside(format_days(after)))

    print u""

    for s in series:
        episodes = ns.series.get_episodes(s)

        if episodes is None:
            print ns.C.warning(u"episodes not synced: {0}".format(
                s['series_name']))
            continue

        result = find_next_episode(
            episodes, ns.series.is_episode_watched,
            ignored_seasons=ns.ignore)

        if result is None:
            all_seen.append(s)
            continue

        next_episode, next_airdate = result
        next_episodes.append((s, next_episode, next_airdate))

    next_episodes = sorted(next_episodes, key=episode_sort_key)
    next_episodes = filter_episodes(ns, next_episodes, specific, now)

    for color, series, episode in next_episodes:
        print color(u"{0} - {1[series_name]} {2}".format(
            format_airdate(episode['first_aired'], now=now),
            series,
            short_episode(episode),
        ))

    if specific or ns.all:
        for s in all_seen:
            print ns.C.all_seen(u"never - {0[series_name]}".format(s))

    return 0


def setup(parser):
    parser.add_argument(
        "series_query",
        metavar="<name|id>",
        nargs='?',
        default=None,
        help="The id or name of the series.",
    )

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
