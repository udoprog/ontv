from ..utils import local_series_finder
from ..utils import with_series
from ..utils import numeric_ranges
from ..format import print_series


@with_series(local_series_finder)
def action(ns, series):
    print ns.term.bold_magenta(u"Details about series")
    print u""

    print_series(
        ns.term, series,
        focused=ns.seasons,
        focused_episodes=ns.episodes,
        series_dao=ns.series)

    return 0


def setup(parser):
    parser.add_argument(
        "series_id",
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

    parser.set_defaults(action=action)
