import textwrap

from .utils import group_episodes


def print_wrapped(term, text, indent=""):
    wrapper = textwrap.TextWrapper()
    wrapper.initial_indent = indent
    wrapper.subsequent_indent = indent

    for line in wrapper.wrap(text):
        print line


def print_title(term, title):
    print term.bold_cyan(title)


def print_episode(term, series_dao, episode,
                  indent="", short_version=True):
    color = term.white

    if series_dao.is_episode_watched(episode):
        color = term.bold_blue

    print color(
        u"{0}{1:02} {2}".format(
            indent, episode['episode_number'], episode['episode_name']))

    if short_version:
        return

    if episode['overview']:
        print_wrapped(term, episode['overview'], indent=indent + "  ")


def print_season(
        term, series_dao, season, episodes,
        short_version=True, active_episode=None, indent=""):

    all_watched = all(
        map(series_dao.is_episode_watched, episodes))

    color = term.white

    if all_watched:
        color = term.bold_blue

    print color(u"  Season {0}".format(season))

    if short_version:
        return

    for episode in episodes:
        if active_episode is not None:
            if active_episode != episode['episode_number']:
                continue

        print_episode(
            term, series_dao, episode,
            short_version=(active_episode is None),
            indent=indent + "  ")


def print_series(
    term, series,
    short_version=False,
    active_season=None,
    active_episode=None,
    series_dao=None
):
    print_title(term, u"{0} (id: {1})".format(
        series['series_name'], series['series_id']))

    if 'first_aired' in series:
        print u"{t.cyan}First Aired:{t.normal} {0}".format(
            series['first_aired'], t=term)

    if 'overview' in series:
        if series['overview']:
            print_wrapped(term, series['overview'], indent="  ")

    if short_version:
        return

    if 'actors' in series:
        print term.cyan(u"Actors:")

        for actor in series['actors']:
            print u" - {0}".format(actor)

    print term.cyan(u"Seasons")

    episodes = series_dao.get_episodes(series)
    seasons = group_episodes(episodes)

    for season_number, season_episodes in sorted(seasons.items()):
        if active_season is not None:
            if active_season != season_number:
                continue

        print_season(
            term, series_dao, season_number, season_episodes,
            short_version=(active_season is None),
            active_episode=active_episode,
            indent="  ")
