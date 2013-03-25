import datetime

from ..utils import sorted_episodes
from ..format import short_episode
from ..format import format_airdate


def episode_key(now):
    def __episode_key(item):
        season, episode, airdate = item

        if episode is None or airdate is None:
            return 0

        return abs((now - airdate).days)

    return __episode_key


def action(ns):
    # prevent excessive time fetching.
    now = datetime.datetime.now()

    episode_sort_key = episode_key(now)

    next_episodes = list()
    all_seen = list()

    for series in ns.series.list_series():
        episodes = ns.series.get_episodes(series)

        if episodes is None:
            print ns.term.bold_red(u"episodes not synced: {0}".format(
                series['series_name']))
            continue

        next_episode = None
        next_airdate = None

        for episode in sorted_episodes(episodes):
            if episode['season_number'] in ns.ignored_seasons:
                continue

            if ns.series.is_episode_watched(episode):
                continue

            next_episode = episode
            next_airdate = datetime.datetime.strptime(
                episode['first_aired'], "%Y-%m-%d")
            break

        if next_episode is None:
            all_seen.append(series)
            continue

        next_episodes.append((series, next_episode, next_airdate))

    next_episodes = sorted(next_episodes, key=episode_sort_key)

    for series, episode, airdate in next_episodes:
        print ns.term.bold_green(u"{0} {1}".format(
            series['series_name'], short_episode(episode)))
        print ns.term.cyan("  Air date: {0}".format(
            format_airdate(episode['first_aired'], now=now)))

    for series in all_seen:
        print ns.term.green(u"{0}: all seen".format(
            series['series_name']))
        continue

    return 0


def setup(parser):
    parser.add_argument(
        '--ignored-seasons', '-i',
        help="Specify a list of seasons to ignore, defaults to '0'",
        default=set([0]),
        type=lambda s: set(map(int, s.split(','))))
    parser.set_defaults(action=action)
