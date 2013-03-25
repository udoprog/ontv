from ..utils import local_series_finder
from ..utils import with_series
from ..utils import numeric_ranges
from ..utils import find_next_episode


def find_episodes(ns, episodes):
    if ns.next:
        result = find_next_episode(
            episodes, ns.series.is_episode_watched,
            ignored_seasons=ns.ignored_seasons)

        if result is None:
            print ns.term.bold_red(u"no episode is next")
            return

        next_episode, next_airdate = result
        yield next_episode
        return

    for episode in episodes:
        if ns.seasons is not None:
            if episode['season_number'] not in ns.seasons:
                continue

        if ns.episodes is not None:
            if episode['episode_number'] not in ns.episodes:
                continue

        yield episode

    return


@with_series(local_series_finder)
def action(ns, series):
    episodes = ns.series.get_episodes(series)

    changed = 0

    for episode in find_episodes(ns, episodes):
        is_watched = ns.series.is_episode_watched(episode)

        name = u"{0[series_name]} Season {1:02}, Episode {2:02}".format(
            series, episode['season_number'], episode['episode_number'])

        if is_watched and not ns.unmark:
            print ns.term.bold_red(u"already marked: {0}".format(name))
            continue

        if not is_watched and ns.unmark:
            print ns.term.bold_red(u"not marked: {0}".format(name))
            continue

        ns.series.set_episode_watched(episode, (not ns.unmark))

        if not ns.unmark:
            print ns.term.bold_green(u"marked: {0}".format(name))
        else:
            print ns.term.bold_green(u"unmarked: {0}".format(name))

        changed += 1

    if changed == 0:
        print ns.term.bold_red(u"did not change any episodes")
        return 1

    return 0


def setup(parser):
    parser.add_argument(
        "series_id",
        metavar="<name|id>",
        help="The id of the series.",
    )

    parser.add_argument(
        "seasons",
        metavar="seasons",
        nargs='?',
        default=None,
        type=numeric_ranges,
        help="The season of the episode.",
    )

    parser.add_argument(
        "episodes",
        metavar="episodes",
        nargs='?',
        default=None,
        type=numeric_ranges,
        help="The episode number of the season.",
    )

    parser.add_argument(
        "--next",
        default=False,
        help="Mark the next episode not watched.",
        action='store_const',
        const=True,
    )

    parser.add_argument(
        '--ignored-seasons', '-i',
        help="Specify a list of seasons to ignore, defaults to '0'",
        default=set([0]),
        type=numeric_ranges,
    )

    parser.add_argument(
        "--unmark",
        default=False,
        help="The episode number of the season.",
        action='store_const',
        const=True,
    )

    parser.set_defaults(action=action)
