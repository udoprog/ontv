from ..utils import local_series_finder
from ..utils import with_resource
from ..utils import numeric_ranges
from ..utils import group_episodes
from ..utils import local_episodes_finder
from ..format import print_series


@with_resource(local_series_finder)
@with_resource(local_episodes_finder)
def action(ns, series, episodes):
    print ns.term.bold_magenta(u"Details about series")
    print u""

    seasons = group_episodes(episodes)

    print_series(
        ns.term, series,
        seasons=seasons,
        series_dao=ns.series)

    return 0


def setup(parser):
    parser.add_argument(
        "series_query",
        metavar="<name|id>",
        help="The id of the series to add",
    )

    parser.add_argument(
        "seasons",
        nargs="?",
        metavar="[season]",
        type=numeric_ranges,
        help="Filter out the specified season.",
        default=None,
    )

    parser.add_argument(
        "episodes",
        nargs="?",
        metavar="[season]",
        type=numeric_ranges,
        help="Filter out the specified episode.",
        default=None,
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

    parser.set_defaults(action=action)
