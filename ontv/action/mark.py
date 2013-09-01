from ..utils import numeric_ranges
from ..utils import with_resource
from ..utils import local_series_finder
from ..utils import local_episodes_finder


@with_resource(local_series_finder)
@with_resource(local_episodes_finder)
def action(ns, series, episodes):
    changed = 0

    for episode in episodes:
        is_watched = ns.series.is_episode_watched(episode)

        name = u'{0[series_name]} S{1:02}E{2:02}: {3}'.format(
            series,
            episode['season_number'],
            episode['episode_number'],
            episode['episode_name']
        )

        if is_watched and not ns.unmark:
            print ns.t.bold_red(u"Already watched: {0}".format(name))
            continue

        if not is_watched and ns.unmark:
            print ns.t.bold_red(u"Not watched: {0}".format(name))
            continue

        ns.series.set_episode_watched(episode, (not ns.unmark))

        if not ns.unmark:
            print ns.t.bold_green(u"Marked: {0}".format(name))
        else:
            print ns.t.bold_green(u"Unmarked: {0}".format(name))

        changed += 1

    if changed == 0:
        print ns.t.bold_red(u"did not change any episodes")
        return 1

    return 0


def setup(parser):
    parser.add_argument(
        "series_query",
        metavar="<name|id>",
        help="The id or name of the series.",
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
        '--ignored-seasons',
        '-i',
        metavar="<ranges>",
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
