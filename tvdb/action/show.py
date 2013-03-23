from ..format import print_series
from ..utils import find_series


def action(ns):
    series_id = find_series(ns.term, ns.series.find_series, ns.series_id)

    if series_id is None:
        print ns.term.bold_red(u"no such series: {0}".format(ns.series_id))
        return 0

    series = ns.series.get(series_id)

    if series is None:
        print ns.term.bold_red(u"no such series (id): {0}".format(series_id))
        return 0

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
