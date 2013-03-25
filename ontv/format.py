import textwrap
import datetime
from dateutil import relativedelta

from .utils import group_episodes


def short_episode(episode):
    return u"S{0:02}E{1:02}".format(
        episode['season_number'], episode['episode_number'])


def readable_timedelta(now, then, next_day, suffix="", prefix=""):
    delta = relativedelta.relativedelta(now, then)

    years = abs(delta.years)
    months = abs(delta.months)
    days = abs(delta.days)

    if years == 0 and months == 0 and days == 0:
        return "today"

    if years == 0 and months == 0 and days == 1:
        return next_day

    date = list()

    if years != 0:
        if years == 1:
            date.append("1 year")
        else:
            date.append("{0} years".format(years))

    if years < 2 and months != 0:
        if months == 1:
            date.append("1 month")
        else:
            date.append("{0} months".format(months))

    if months == 0:
        if days == 1:
            date.append("1 day")
        else:
            date.append("{0} days".format(days))

    return prefix + ", ".join(date) + suffix


def format_airdate(aired, now=None):
    if now is None:
        now = datetime.datetime.now()

    if aired is None:
        return "n/a"

    try:
        then = datetime.datetime.strptime(aired, "%Y-%m-%d")
    except:
        return "<invalid %Y-%m-%d>"

    if then <= now:
        return readable_timedelta(
            now, then, "yesterday", suffix=" ago")

    return readable_timedelta(
        now, then, "tomorrow", prefix="in ")


def print_wrapped(text, indent=""):
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

    print term.cyan(u"{0}Air date {1}".format(
        indent + "  ", format_airdate(episode['first_aired'])))

    if short_version:
        return

    if episode['overview']:
        print_wrapped(episode['overview'], indent=indent + u"  ")

    if 'guest_stars' in episode:
        print term.cyan(u"{0}Guest stars:".format(indent + u"  "))
        print_wrapped(format_compact_list(episode['guest_stars']),
                      indent=indent + u"    ")


def print_season(
        term, series_dao, season, episodes,
        short_version=True, focused=set(), indent=""):

    all_watched = all(
        map(series_dao.is_episode_watched, episodes))

    color = term.white

    if all_watched:
        color = term.bold_blue

    print color(u"{0}Season {1}".format(indent, season))

    if short_version:
        return

    for episode in episodes:
        if focused and episode['episode_number'] not in focused:
            continue

        print_episode(
            term, series_dao, episode,
            short_version=(not bool(focused)),
            indent=indent + "  ")


def print_list(items, item_format=u"- {0}", indent=""):
    if items is None:
        print u"{0}(empty)".format(indent)
        return

    for item in items:
        print u"{0}{1}".format(indent, item_format.format(item))


def format_compact_list(items, item_format=u"{0}"):
    if items is None:
        return u"(empty)"

    return u", ".join(map(item_format.format, items))


def print_series(
    term, series,
    short_version=False,
    focused=set(),
    focused_episodes=set(),
    series_dao=None,
    indent=u"",
):
    print_title(term, u"{0} (id: {1})".format(
        series['series_name'], series['series_id']))

    if 'first_aired' in series:
        print term.cyan(u"Air date {0}".format(
            format_airdate(series['first_aired'])))

    if 'overview' in series:
        if series['overview']:
            print_wrapped(series['overview'], indent="  ")

    if short_version:
        return

    if 'actors' in series:
        print term.cyan(u"{0}Actors:".format(indent))
        print_wrapped(format_compact_list(series['actors']),
                      indent=indent + u"  ")

    print term.cyan(u"Seasons")

    episodes = series_dao.get_episodes(series)
    seasons = group_episodes(episodes)

    for season_number, season_episodes in sorted(seasons.items()):
        if focused and season_number not in focused:
            continue

        print_season(
            term, series_dao, season_number, season_episodes,
            short_version=(not bool(focused)),
            focused=focused_episodes,
            indent="  ")
