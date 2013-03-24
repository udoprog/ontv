from ..utils import local_series_finder
from ..utils import with_series
from ..format import print_series


@with_series(local_series_finder)
def action(ns, series):
    print_series(
        ns.term, series,
        active_season=ns.season_number,
        active_episode=ns.episode_number,
        series_dao=ns.series)

    return 0


def setup(parser):
    parser.add_argument(
        "series_id",
        metavar="<name|id>",
        help="The id of the series to add",
    )

    parser.add_argument(
        "season_number",
        nargs="?",
        metavar="[season]",
        type=int,
        help="Filter out the specified season.",
        default=None,
    )

    parser.add_argument(
        "episode_number",
        nargs="?",
        metavar="[season]",
        type=int,
        help="Filter out the specified episode.",
        default=None,
    )

    parser.set_defaults(action=action)
