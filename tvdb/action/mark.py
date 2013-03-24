from ..utils import group_episodes
from ..utils import local_series_finder
from ..utils import with_series


@with_series(local_series_finder)
def action(ns, series):
    episodes = ns.series.get_episodes(series)

    seasons = group_episodes(episodes)

    episodes = seasons.get(ns.season_number, None)

    if episodes is None:
        print ns.term.bold_red(u"no such season: {0}".format(ns.season_number))
        return 1

    changed = 0

    for episode in episodes:
        if ns.episode_number is not None:
            if episode['episode_number'] != ns.episode_number:
                continue

        name = u"{0[series_name]} Season {1:02}, Episode {2:02}".format(
            series, episode['season_number'], episode['episode_number'])

        if ns.series.is_episode_watched(episode):
            if not ns.unmark:
                print ns.term.bold_red(u"already marked: {0}".format(name))
                continue
        else:
            if ns.unmark:
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
        "season_number",
        metavar="<season>",
        type=int,
        help="The season of the episode.",
    )

    parser.add_argument(
        "episode_number",
        metavar="episode",
        nargs='?',
        default=None,
        type=int,
        help="The episode number of the season.",
    )

    parser.add_argument(
        "--unmark",
        default=False,
        help="The episode number of the season.",
        action='store_const',
        const=True,
    )

    parser.set_defaults(action=action)
